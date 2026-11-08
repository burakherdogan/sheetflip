//! HTML templates. `{{name}}` is filled from the given values, or else from the translations.

use crate::i18n::Lang;
use axum::response::Html;

pub fn render(tpl: &str, lang: Lang, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(tpl.len());
    let mut rest = tpl;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let end = rest[start..].find("}}").expect("unclosed {{ in template") + start;
        let name = &rest[start + 2..end];
        match vars.iter().find(|(k, _)| *k == name) {
            Some((_, v)) => out.push_str(v),
            None => out.push_str(lang.t(name)),
        }
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    out
}

pub fn page(lang: Lang, title: &str, body: &str) -> Html<String> {
    let (tr, en) = if lang == Lang::Tr {
        ("active", "")
    } else {
        ("", "active")
    };
    Html(render(
        include_str!("ui/layout.html"),
        lang,
        &[
            ("lang", lang.code()),
            ("title", title),
            ("body", body),
            ("tr_active", tr),
            ("en_active", en),
            ("js_strings", &lang.js_strings()),
        ],
    ))
}

pub fn error(lang: Lang, msg: &str) -> Html<String> {
    let body = render(
        include_str!("ui/error.html"),
        lang,
        &[("message", &esc(msg))],
    );
    page(lang, lang.t("error_title"), &body)
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Progress bar over the print steps; `active` is the current step index.
pub fn steps(lang: Lang, duplex: bool, active: usize) -> String {
    let keys: &[&str] = if duplex {
        &[
            "step_setup",
            "step_front",
            "step_flip",
            "step_back",
            "step_done",
        ]
    } else {
        &["step_setup", "step_print", "step_done"]
    };
    let items: String = keys
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let class = if i < active {
                "done"
            } else if i == active {
                "active"
            } else {
                ""
            };
            format!(r#"<li class="{class}"><b>{}</b>{}</li>"#, i + 1, lang.t(k))
        })
        .collect();
    format!(r#"<ol class="steps">{items}</ol>"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATES: &[&str] = &[
        include_str!("ui/layout.html"),
        include_str!("ui/index.html"),
        include_str!("ui/summary.html"),
        include_str!("ui/front.html"),
        include_str!("ui/done.html"),
        include_str!("ui/error.html"),
        include_str!("ui/message.html"),
        include_str!("ui/printer_test.html"),
        include_str!("ui/test_check.html"),
    ];

    /// Placeholders filled by the code rather than from the translations.
    const VALUES: &[&str] = &[
        "lang",
        "title",
        "body",
        "tr_active",
        "en_active",
        "js_strings",
        "printer_options",
        "tested_json",
        "steps",
        "sheet_count",
        "copies_note",
        "load_paper_text",
        "side_rows",
        "printer_name",
        "mode_label",
        "document_pages",
        "color_label",
        "quality_label",
        "edge_row",
        "numbers_label",
        "id",
        "confirm",
        "button",
        "wait_text",
        "flip_1_text",
        "status",
        "finish",
        "message",
        "heading",
        "primary",
        "primary_href",
        "secondary",
        "secondary_href",
        "tested_text",
        "per_side",
        "max_per_side",
    ];

    #[test]
    fn every_template_key_is_translated() {
        for tpl in TEMPLATES {
            let mut rest = *tpl;
            while let Some(start) = rest.find("{{") {
                let end = rest[start..].find("}}").unwrap() + start;
                let name = &rest[start + 2..end];
                if !VALUES.contains(&name) {
                    for lang in [Lang::Tr, Lang::En] {
                        lang.t(name);
                    }
                }
                rest = &rest[end + 2..];
            }
        }
    }

    #[test]
    fn browser_strings_are_valid_json() {
        for lang in [Lang::Tr, Lang::En] {
            let v: serde_json::Value = serde_json::from_str(&lang.js_strings()).unwrap();
            assert!(v.as_object().unwrap().values().all(|s| s.is_string()));
        }
    }

    #[test]
    fn render_fills_values_and_translations() {
        let out = render("<p>{{x}} {{cancel}}</p>", Lang::En, &[("x", "1")]);
        assert_eq!(out, "<p>1 Cancel</p>");
    }
}
