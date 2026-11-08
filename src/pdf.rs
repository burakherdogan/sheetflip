//! Page selection, sheet layout and PDF generation.

use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Single,
    Duplex,
    Booklet,
}

/// Four A4 pages (two sheets) for the printer test: a large page number and an arrow pointing to the top.
/// `top` and `back` are the labels; `back` contains `{n}` for the page expected on the other side.
pub fn test_pdf(top: &str, back: &str) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding",
    });
    let kids: Vec<Object> = (1..=4)
        .map(|n: u32| {
            let other = if n % 2 == 1 { n + 1 } else { n - 1 };
            let label = back.replace("{n}", &other.to_string());
            let ops = format!(
                "0.2 g 297.5 760 m 267.5 715 l 327.5 715 l f 287.5 640 20 75 re f \
                 BT /F 22 Tf {tx:.1} 655 Td ({top}) Tj ET \
                 BT /F 260 Tf {nx:.1} 330 Td ({n}) Tj ET \
                 BT /F 18 Tf {bx:.1} 200 Td ({label}) Tj ET \
                 0.6 g BT /F 9 Tf 262 40 Td (SHEETFLIP TEST) Tj ET",
                top = pdf_text(top),
                tx = 335.0,
                nx = 297.5 - 260.0 * 0.556 / 2.0,
                bx = 297.5 - label.chars().count() as f32 * 18.0 * 0.6 / 2.0,
                label = pdf_text(&label),
            );
            let contents = doc.add_object(Stream::new(dictionary! {}, ops.into_bytes()));
            let page = doc.add_object(dictionary! {
                "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
                "Resources" => dictionary! { "Font" => dictionary! { "F" => font } }, "Contents" => contents,
            });
            page.into()
        })
        .collect();
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => 4 }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut out = Vec::new();
    doc.save_to(&mut out)
        .expect("writing to memory cannot fail");
    out
}

/// PDF literal string body in WinAnsi encoding (covers Turkish Ç, Ö, Ü; not ğ, ı, ş).
fn pdf_text(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '(' | ')' | '\\' => format!("\\{c}"),
            c if c.is_ascii() => c.to_string(),
            c if (c as u32) < 256 => format!("\\{:o}", c as u32),
            _ => "?".into(),
        })
        .collect()
}

/// "1-3, 5, 8-" -> 0-based indices, in order. Empty means all pages.
pub fn parse_pages(spec: &str, n: usize) -> Option<Vec<usize>> {
    if spec.trim().is_empty() {
        return Some((0..n).collect());
    }
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let a: usize = if a.is_empty() { 1 } else { a.parse().ok()? };
        let b: usize = if b.is_empty() { n } else { b.parse().ok()? };
        if a < 1 || b > n || a > b {
            return None;
        }
        out.extend(a - 1..b);
    }
    (!out.is_empty()).then_some(out)
}

/// Page order per side, `per_side` pages on each side of a sheet. `None` is a blank page.
/// Booklets always have two pages per side.
pub fn layout(
    mode: Mode,
    per_side: usize,
    pages: &[usize],
) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
    let mut p: Vec<Option<usize>> = pages.iter().copied().map(Some).collect();
    let pad = |p: &mut Vec<Option<usize>>, multiple: usize| {
        p.resize(p.len().div_ceil(multiple) * multiple, None)
    };
    match mode {
        Mode::Single => {
            pad(&mut p, per_side);
            (p, vec![])
        }
        Mode::Duplex => {
            pad(&mut p, 2 * per_side);
            let sides: Vec<&[Option<usize>]> = p.chunks(per_side).collect();
            let front = sides
                .iter()
                .step_by(2)
                .flat_map(|s| s.iter().copied())
                .collect();
            let back = sides
                .iter()
                .skip(1)
                .step_by(2)
                .flat_map(|s| s.iter().copied())
                .collect();
            (front, back)
        }
        Mode::Booklet => {
            pad(&mut p, 4);
            let n = p.len();
            let (mut front, mut back) = (vec![], vec![]);
            for s in 0..n / 4 {
                front.extend([p[n - 1 - 2 * s], p[2 * s]]);
                back.extend([p[2 * s + 1], p[n - 2 - 2 * s]]);
            }
            (front, back)
        }
    }
}

/// Columns and rows used for row-major placement on an A4 sheet.
pub fn grid(n: usize) -> (usize, usize) {
    let columns = (n.max(1) as f64).sqrt().ceil() as usize;
    (columns, n.max(1).div_ceil(columns))
}

/// Write a PDF containing the given pages of `src` (blank where `None`).
pub fn build_pdf(
    src: &Path,
    order: &[Option<usize>],
    per_side: usize,
    rotate: bool,
    numbers: bool,
    out: &Path,
) -> lopdf::Result<()> {
    let mut doc = Document::load(src)?;
    let ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
    for &id in &ids {
        flatten_inherited(&mut doc, id);
    }
    let media = doc
        .get_dictionary(ids[0])
        .ok()
        .and_then(|d| d.get(b"MediaBox").ok().cloned())
        .unwrap_or_else(|| vec![0.into(), 0.into(), 595.into(), 842.into()].into());
    if numbers {
        let font = doc.add_object(
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
        );
        let mut stamped = std::collections::HashSet::new();
        for i in order.iter().flatten() {
            if stamped.insert(*i) {
                stamp_number(&mut doc, ids[*i], *i + 1, font)?;
            }
        }
    }
    let pages_id = doc.new_object_id();
    let kids = if per_side > 1 {
        impose(&mut doc, &ids, order, per_side)?
    } else {
        let mut kids = Vec::new();
        for slot in order {
            let id = match slot {
                Some(i) => {
                    let copy = doc.get_object(ids[*i])?.clone();
                    doc.add_object(copy)
                }
                None => {
                    let contents = doc.add_object(Stream::new(dictionary! {}, vec![]));
                    doc.add_object(dictionary! {
                        "Type" => "Page", "MediaBox" => media.clone(),
                        "Resources" => dictionary! {}, "Contents" => contents,
                    })
                }
            };
            kids.push(id);
        }
        kids
    };
    for k in &kids {
        let d = doc.get_dictionary_mut(*k)?;
        d.set("Parent", pages_id);
        if rotate {
            let r = d
                .get(b"Rotate")
                .ok()
                .and_then(|r| r.as_i64().ok())
                .unwrap_or(0);
            d.set("Rotate", (r + 180).rem_euclid(360));
        }
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Count" => count,
            "Kids" => kids.into_iter().map(Object::Reference).collect::<Vec<_>>(),
        }),
    );
    doc.catalog_mut()?.set("Pages", pages_id);
    doc.prune_objects();
    doc.save(out)?;
    Ok(())
}

/// Resolve inherited page geometry after flattening the page tree.
fn page_box(doc: &Document, page: ObjectId) -> lopdf::Result<Vec<f32>> {
    let d = doc.get_dictionary(page)?;
    let value = d.get(b"CropBox").or_else(|_| d.get(b"MediaBox"))?;
    let values = doc.dereference(value)?.1.as_array()?;
    values
        .iter()
        .map(|v| Ok(num(doc.dereference(v)?.1)))
        .collect()
}

fn impose(
    doc: &mut Document,
    ids: &[ObjectId],
    order: &[Option<usize>],
    per_side: usize,
) -> lopdf::Result<Vec<ObjectId>> {
    let (cols, rows) = grid(per_side);
    let (width, height) = if cols > rows {
        (842.0, 595.0)
    } else {
        (595.0, 842.0)
    };
    let (cw, ch) = (width / cols as f32, height / rows as f32);
    let mut forms = std::collections::HashMap::new();
    let mut sheets = Vec::new();
    for slots in order.chunks(per_side) {
        let mut xobjects = Dictionary::new();
        let mut content = String::new();
        for (slot, index) in slots.iter().enumerate() {
            let Some(index) = index else { continue };
            let (form, w, h) = if let Some(value) = forms.get(index) {
                *value
            } else {
                let page = ids[*index];
                let bounds = page_box(doc, page)?;
                let (x0, y0, x1, y1) = (bounds[0], bounds[1], bounds[2], bounds[3]);
                let d = doc.get_dictionary(page)?;
                let rotation = d
                    .get(b"Rotate")
                    .ok()
                    .and_then(|r| doc.dereference(r).ok())
                    .and_then(|(_, r)| r.as_i64().ok())
                    .unwrap_or(0)
                    .rem_euclid(360);
                let resources = d
                    .get(b"Resources")
                    .cloned()
                    .unwrap_or_else(|_| dictionary! {}.into());
                let (matrix, w, h) = match rotation {
                    90 => ([0., -1., 1., 0., -y0, x1], y1 - y0, x1 - x0),
                    180 => ([-1., 0., 0., -1., x1, y1], x1 - x0, y1 - y0),
                    270 => ([0., 1., -1., 0., y1, -x0], y1 - y0, x1 - x0),
                    _ => ([1., 0., 0., 1., -x0, -y0], x1 - x0, y1 - y0),
                };
                let bytes = doc.get_page_content(page)?;
                let form = doc.add_object(Stream::new(
                    dictionary! {
                        "Type" => "XObject", "Subtype" => "Form", "FormType" => 1,
                        "BBox" => bounds.into_iter().map(Object::Real).collect::<Vec<_>>(),
                        "Matrix" => matrix.into_iter().map(Object::Real).collect::<Vec<_>>(),
                        "Resources" => resources,
                    },
                    bytes,
                ));
                forms.insert(*index, (form, w, h));
                (form, w, h)
            };
            let scale = (cw / w).min(ch / h);
            let x = (slot % cols) as f32 * cw + (cw - w * scale) / 2.;
            let y = height - (slot / cols + 1) as f32 * ch + (ch - h * scale) / 2.;
            let name = format!("P{slot}");
            xobjects.set(name.as_str(), form);
            content.push_str(&format!("q {scale} 0 0 {scale} {x} {y} cm /{name} Do Q\n"));
        }
        let contents = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        sheets.push(doc.add_object(dictionary! {
            "Type" => "Page", "MediaBox" => vec![0.into(), 0.into(), Object::Real(width), Object::Real(height)],
            "Resources" => dictionary! { "XObject" => xobjects }, "Contents" => contents,
        }));
    }
    Ok(sheets)
}

fn num(o: &Object) -> f32 {
    match o {
        Object::Integer(i) => *i as f32,
        Object::Real(r) => *r,
        _ => 0.0,
    }
}

/// Small grey page number at the bottom centre.
fn stamp_number(doc: &mut Document, page: ObjectId, n: usize, font: ObjectId) -> lopdf::Result<()> {
    let d = doc.get_dictionary(page)?.clone();
    let deref = |o: Option<&Object>| -> Dictionary {
        match o {
            Some(Object::Reference(r)) => doc.get_dictionary(*r).cloned().unwrap_or_default(),
            Some(Object::Dictionary(d)) => d.clone(),
            _ => Dictionary::new(),
        }
    };
    let mut res = deref(d.get(b"Resources").ok());
    let mut fonts = deref(res.get(b"Font").ok());
    fonts.set("FPN", font);
    res.set("Font", fonts);
    let mb: Vec<f32> = d
        .get(b"MediaBox")
        .and_then(|m| m.as_array())
        .map(|a| a.iter().map(num).collect())
        .unwrap_or(vec![0.0, 0.0, 595.0, 842.0]);
    let text = n.to_string();
    let x = (mb[0] + mb[2]) / 2.0 - text.len() as f32 * 8.0 * 0.556 / 2.0;
    let y = mb[1] + 18.0;
    let mut contents: Vec<Object> = match d.get(b"Contents") {
        Ok(Object::Array(a)) => a.clone(),
        Ok(o) => vec![o.clone()],
        Err(_) => vec![],
    };
    let open = doc.add_object(Stream::new(dictionary! {}, b"q\n".to_vec()));
    let mark = format!("\nQ q 0.4 g BT /FPN 8 Tf {x:.1} {y:.1} Td ({text}) Tj ET Q\n");
    let close = doc.add_object(Stream::new(dictionary! {}, mark.into_bytes()));
    contents.insert(0, open.into());
    contents.push(close.into());
    let d = doc.get_dictionary_mut(page)?;
    d.set("Resources", res);
    d.set("Contents", contents);
    Ok(())
}

fn flatten_inherited(doc: &mut Document, id: ObjectId) {
    let keys: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];
    let mut found: Dictionary = Dictionary::new();
    let mut parent = doc
        .get_dictionary(id)
        .ok()
        .and_then(|d| d.get(b"Parent").ok()?.as_reference().ok());
    while let Some(pid) = parent {
        let Ok(pd) = doc.get_dictionary(pid) else {
            break;
        };
        for k in keys {
            if !found.has(k)
                && let Ok(v) = pd.get(k)
            {
                found.set(k, v.clone());
            }
        }
        parent = pd.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
    }
    if let Ok(d) = doc.get_dictionary_mut(id) {
        for (k, v) in found.into_iter() {
            if !d.has(&k) {
                d.set(k, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbitrary_n_up_keeps_vector_forms_and_empty_slots() {
        for n in [3, 5, 6] {
            let mut doc = Document::load_mem(&test_pdf("TOP", "BACK {n}")).unwrap();
            let ids: Vec<_> = doc.get_pages().into_values().collect();
            let mut order = vec![Some(0); n + 1];
            order[1] = None;
            let sheets = impose(&mut doc, &ids, &order, n).unwrap();
            assert_eq!(sheets.len(), 2);
            let (cols, rows) = grid(n);
            assert_eq!((cols, rows), if n == 3 { (2, 2) } else { (3, 2) });
            let first = doc.get_dictionary(sheets[0]).unwrap();
            let bounds = first.get(b"MediaBox").unwrap().as_array().unwrap();
            assert_eq!(num(&bounds[2]), if n == 3 { 595. } else { 842. });
            let ops =
                lopdf::content::Content::decode(&doc.get_page_content(sheets[0]).unwrap()).unwrap();
            let names: Vec<_> = ops
                .operations
                .iter()
                .filter(|o| o.operator == "Do")
                .map(|o| o.operands[0].as_name().unwrap())
                .collect();
            assert_eq!(names.len(), n - 1);
            assert_eq!(names[0], b"P0");
            assert_eq!(names[1], b"P2");
            let res = first.get(b"Resources").unwrap().as_dict().unwrap();
            let objects = res.get(b"XObject").unwrap().as_dict().unwrap();
            let form = doc
                .get_object(objects.get(b"P0").unwrap().as_reference().unwrap())
                .unwrap()
                .as_stream()
                .unwrap();
            assert_eq!(
                form.dict.get(b"Subtype").unwrap().as_name().unwrap(),
                b"Form"
            );
            assert_eq!(form.content, doc.get_page_content(ids[0]).unwrap());
            let second = String::from_utf8(doc.get_page_content(sheets[1]).unwrap()).unwrap();
            assert_eq!(second.matches(" Do ").count(), 1);
        }
    }

    #[test]
    fn inherited_crop_and_rotation_normalize_form_coordinates() {
        for (rotation, expected) in [
            (0, [1., 0., 0., 1., -10., -20.]),
            (90, [0., -1., 1., 0., -20., 210.]),
            (180, [-1., 0., 0., -1., 210., 420.]),
            (270, [0., 1., -1., 0., 420., -10.]),
        ] {
            let mut doc = Document::load_mem(&test_pdf("TOP", "BACK {n}")).unwrap();
            let page = doc.get_pages()[&1];
            let parent = doc
                .get_dictionary(page)
                .unwrap()
                .get(b"Parent")
                .unwrap()
                .as_reference()
                .unwrap();
            let crop = doc.add_object(vec![10.into(), 20.into(), 210.into(), 420.into()]);
            let d = doc.get_dictionary_mut(parent).unwrap();
            d.set("CropBox", crop);
            d.set("Rotate", rotation);
            flatten_inherited(&mut doc, page);
            let sheets = impose(&mut doc, &[page], &[Some(0)], 3).unwrap();
            let sheet = doc.get_dictionary(sheets[0]).unwrap();
            let objects = sheet
                .get(b"Resources")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"XObject")
                .unwrap()
                .as_dict()
                .unwrap();
            let form = doc
                .get_object(objects.get(b"P0").unwrap().as_reference().unwrap())
                .unwrap()
                .as_stream()
                .unwrap();
            let matrix: Vec<_> = form
                .dict
                .get(b"Matrix")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(num)
                .collect();
            assert_eq!(matrix, expected);
            let ops =
                lopdf::content::Content::decode(&doc.get_page_content(sheets[0]).unwrap()).unwrap();
            let cm = ops.operations.iter().find(|o| o.operator == "cm").unwrap();
            let expected_scale = if rotation % 180 == 0 {
                (297.5_f32 / 200.).min(421. / 400.)
            } else {
                (297.5_f32 / 400.).min(421. / 200.)
            };
            assert!((num(&cm.operands[0]) - expected_scale).abs() < 0.0001);
        }
    }

    #[test]
    fn repeated_pages_are_stamped_once_and_sheets_rotate() {
        let dir = std::env::temp_dir().join(format!("sheetflip-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.pdf");
        let output = dir.join("output.pdf");
        std::fs::write(&source, test_pdf("TOP", "BACK {n}")).unwrap();
        for per_side in [1, 3] {
            build_pdf(
                &source,
                &[Some(0), Some(0), Some(0)],
                per_side,
                true,
                true,
                &output,
            )
            .unwrap();
            let doc = Document::load(&output).unwrap();
            assert_eq!(doc.get_pages().len(), if per_side == 1 { 3 } else { 1 });
            for page in doc.get_pages().into_values() {
                assert_eq!(
                    doc.get_dictionary(page)
                        .unwrap()
                        .get(b"Rotate")
                        .unwrap()
                        .as_i64()
                        .unwrap(),
                    180
                );
                if per_side == 1 {
                    let contents = String::from_utf8(doc.get_page_content(page).unwrap()).unwrap();
                    assert_eq!(contents.matches("/FPN 8 Tf").count(), 1);
                }
            }
            if per_side > 1 {
                let forms: Vec<_> = doc
                    .objects
                    .values()
                    .filter_map(|o| o.as_stream().ok())
                    .filter(|s| {
                        s.dict.get(b"Subtype").ok().and_then(|v| v.as_name().ok()) == Some(b"Form")
                    })
                    .collect();
                assert_eq!(forms.len(), 1);
                assert_eq!(
                    String::from_utf8_lossy(&forms[0].content)
                        .matches("/FPN 8 Tf")
                        .count(),
                    1
                );
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pages() {
        assert_eq!(parse_pages("", 3), Some(vec![0, 1, 2]));
        assert_eq!(parse_pages("1-2, 5", 5), Some(vec![0, 1, 4]));
        assert_eq!(parse_pages("4-", 5), Some(vec![3, 4]));
        assert_eq!(parse_pages("6", 5), None);
    }

    #[test]
    fn booklet_8() {
        let (f, b) = layout(Mode::Booklet, 2, &(0..8).collect::<Vec<_>>());
        let s = |v: Vec<Option<usize>>| {
            v.into_iter()
                .map(|x| x.map_or(0, |i| i + 1))
                .collect::<Vec<_>>()
        };
        assert_eq!(s(f), vec![8, 1, 6, 3]);
        assert_eq!(s(b), vec![2, 7, 4, 5]);
    }

    #[test]
    fn duplex_four_per_side() {
        let (f, b) = layout(Mode::Duplex, 4, &(0..10).collect::<Vec<_>>());
        let s = |v: Vec<Option<usize>>| {
            v.into_iter()
                .map(|x| x.map_or(0, |i| i + 1))
                .collect::<Vec<_>>()
        };
        assert_eq!(s(f), vec![1, 2, 3, 4, 9, 10, 0, 0]);
        assert_eq!(s(b), vec![5, 6, 7, 8, 0, 0, 0, 0]);
    }

    #[test]
    fn duplex_odd_pads_back() {
        let (f, b) = layout(Mode::Duplex, 1, &[0, 1, 2]);
        assert_eq!(f, vec![Some(0), Some(2)]);
        assert_eq!(b, vec![Some(1), None]);
    }
}
