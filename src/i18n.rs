//! Turkish and English UI strings.

use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, header, request::Parts},
};
use std::{collections::BTreeMap, convert::Infallible};

#[derive(Clone, Copy, PartialEq)]
pub enum Lang {
    Tr,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Tr => "tr",
            Lang::En => "en",
        }
    }

    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "tr" => Some(Lang::Tr),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// The `lang` cookie wins; otherwise the browser's preferred language.
    pub fn from_headers(h: &HeaderMap) -> Lang {
        let cookie = h
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let chosen = cookie
            .split(';')
            .filter_map(|c| c.trim().strip_prefix("lang="))
            .find_map(Lang::parse);
        chosen.unwrap_or_else(|| {
            let accept = h
                .get(header::ACCEPT_LANGUAGE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if accept.trim_start().starts_with("tr") {
                Lang::Tr
            } else {
                Lang::En
            }
        })
    }

    pub fn t(self, key: &str) -> &'static str {
        STRINGS
            .iter()
            .find(|(k, ..)| *k == key)
            .map(|(_, tr, en)| if self == Lang::Tr { *tr } else { *en })
            .unwrap_or_else(|| panic!("missing translation: {key}"))
    }

    /// Translation with `{name}` placeholders filled in.
    pub fn f(self, key: &str, vars: &[(&str, &str)]) -> String {
        vars.iter().fold(self.t(key).to_string(), |s, (k, v)| {
            s.replace(&format!("{{{k}}}"), v)
        })
    }

    /// Strings the browser script needs, as a JSON object.
    pub fn js_strings(self) -> String {
        let map: BTreeMap<&str, &str> = JS_KEYS.iter().map(|k| (*k, self.t(k))).collect();
        serde_json::to_string(&map).expect("string map serializes")
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Lang {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Lang::from_headers(&parts.headers))
    }
}

const JS_KEYS: &[&str] = &[
    "js_page_list",
    "js_blank",
    "js_sheet",
    "js_summary",
    "js_copies",
    "js_more",
    "js_not_pdf",
    "js_bad_pdf",
    "js_front",
    "js_back",
    "side_single",
    "err_range",
];

const STRINGS: &[(&str, &str, &str)] = &[
    ("pages_per_side", "Yüz başına sayfa", "Pages per side"),
    ("nav_test", "Yazıcı testi", "Printer test"),
    ("test_title", "Yazıcı testi", "Printer test"),
    (
        "test_intro",
        "Her yazıcı kağıdı farklı alır. 2 kağıtlık bir test baskısıyla Sheetflip, bu yazıcıda arka yüzleri nasıl basacağını öğrenir. Her yazıcı için bir kez yapmanız yeterli.",
        "Every printer feeds paper differently. A two-sheet test print teaches Sheetflip how to print the backs on this printer. You only need to do it once per printer.",
    ),
    (
        "test_step_1",
        "Ön yüzler basılır.",
        "The fronts are printed.",
    ),
    (
        "test_step_2",
        "Kağıtları ekrandaki gibi geri koyarsınız ve arka yüzler basılır.",
        "You reload the paper as shown and the backs are printed.",
    ),
    (
        "test_step_3",
        "İki soruyu cevaplarsınız. Ayar bu yazıcı için kaydedilir.",
        "You answer two questions. The setting is saved for this printer.",
    ),
    ("test_start", "Test sayfasını bas", "Print test page"),
    (
        "test_confirm",
        "Test sayfasının ön yüzleri seçilen yazıcıya gönderilecek (2 kağıt).",
        "The test fronts will be sent to the selected printer (2 sheets).",
    ),
    (
        "tested_list",
        "Test edilmiş yazıcılar: {list}",
        "Tested printers: {list}",
    ),
    ("check_title", "Sonucu kontrol edin", "Check the result"),
    (
        "q_back",
        "1 yazan kağıdın arkasında hangi sayı var?",
        "Which number is on the back of the sheet marked 1?",
    ),
    ("a_none", "Hiçbiri, arkası boş", "None, the back is blank"),
    (
        "q_arrow",
        "Kağıdı kitap sayfası gibi yandan çevirin. Arkadaki ok yukarıyı mı gösteriyor?",
        "Turn the sheet over sideways like a book page. Does the arrow on the back point up?",
    ),
    ("a_up", "Evet, yukarı", "Yes, up"),
    ("a_down", "Hayır, aşağı", "No, down"),
    ("save_setting", "Ayarı kaydet", "Save setting"),
    ("test_saved_title", "Yazıcı hazır", "Printer is ready"),
    (
        "test_saved",
        "<b>{printer}</b> için ayar kaydedildi. Arkalı önlü ve kitapçık baskılar artık bu ayarla yapılır.",
        "The setting for <b>{printer}</b> is saved. Double-sided and booklet prints now use it.",
    ),
    (
        "test_blank_back",
        "Arka yüzler, ön yüzlerle aynı tarafa basılmış. Deste ters çevrilmeden geri konmuş olabilir. Testi tekrarlayın ve desteyi çevirmeyi unutmayın.",
        "The backs were printed on the same side as the fronts. The stack was probably put back without turning it over. Run the test again and turn the stack over.",
    ),
    ("retry", "Testi tekrarla", "Run the test again"),
    (
        "untested_notice",
        "Bu yazıcı henüz test edilmedi.",
        "This printer has not been tested yet.",
    ),
    ("test_link", "Test sayfası basın", "Print a test page"),
    ("test_top", "ÜST", "TOP"),
    ("test_back", "Arkada: {n}", "Back: {n}"),
    ("nav_docs", "Belgeler", "Docs"),
    ("theme", "Temayı değiştir", "Toggle theme"),
    ("language", "Dil", "Language"),
    ("confirm_title", "Yazdırılsın mı?", "Start printing?"),
    ("cancel", "Vazgeç", "Cancel"),
    ("confirm_yes", "Yazdır", "Print"),
    ("preview", "Önizleme", "Preview"),
    (
        "drop_title",
        "PDF dosyasını buraya bırakın",
        "Drop a PDF here",
    ),
    (
        "drop_sub",
        "veya seçmek için tıklayın",
        "or click to choose a file",
    ),
    ("change", "Değiştir", "Change"),
    ("open_original", "Orijinali aç", "Open original"),
    ("settings", "Ayarlar", "Settings"),
    ("printer", "Yazıcı", "Printer"),
    ("layout", "Baskı şekli", "Layout"),
    ("mode_single", "Tek yüz", "Single-sided"),
    ("mode_duplex", "Arkalı önlü", "Double-sided"),
    ("mode_booklet", "Kitapçık", "Booklet"),
    (
        "mode_booklet_sub",
        "A5, ortadan katlanır",
        "A5, folded in half",
    ),
    (
        "mode_duplex_sub",
        "Kağıdı elle çevirerek",
        "With manual paper flip",
    ),
    ("pages", "Sayfalar", "Pages"),
    ("pages_all", "Tümü (örn. 1-5, 8)", "All (e.g. 1-5, 8)"),
    ("copies", "Kopya", "Copies"),
    ("flip_edge", "Çevirme kenarı", "Flip on"),
    ("edge_long", "Uzun kenar", "Long edge"),
    ("edge_short", "Kısa kenar", "Short edge"),
    (
        "booklet_edge_note",
        "Kitapçık her zaman kısa kenardan çevrilir.",
        "Booklets always flip on the short edge.",
    ),
    ("quality", "Kalite", "Quality"),
    ("q_draft", "Taslak", "Draft"),
    ("q_normal", "Normal", "Normal"),
    ("q_high", "Yüksek", "High"),
    ("grayscale", "Siyah-beyaz", "Grayscale"),
    ("page_numbers", "Sayfa numaraları", "Page numbers"),
    ("review", "Devam", "Continue"),
    ("step_setup", "Hazırlık", "Setup"),
    ("step_front", "Ön yüzler", "Fronts"),
    ("step_flip", "Çevir", "Reload"),
    ("step_back", "Arka yüzler", "Backs"),
    ("step_print", "Yazdır", "Print"),
    ("step_done", "Bitti", "Done"),
    ("summary_title", "Baskı özeti", "Print summary"),
    ("sheets", "kağıt", "sheets"),
    (
        "load_paper",
        "Tepsiye en az <strong>{n} kağıt</strong> koyun.",
        "Load at least <strong>{n} sheets</strong>.",
    ),
    ("side_front", "Ön yüzler", "Fronts"),
    ("side_back", "Arka yüzler", "Backs"),
    ("side_single", "Tek yüz", "Single side"),
    ("blank", "boş", "blank"),
    ("js_front", "Ön yüz", "Front"),
    ("js_back", "Arka yüz", "Back"),
    ("document", "Belge", "Document"),
    ("n_pages", "{n} sayfa", "{n} pages"),
    ("color", "Renk", "Color"),
    ("color_color", "Renkli", "Color"),
    ("on", "Açık", "On"),
    ("off", "Kapalı", "Off"),
    ("back_link", "Geri", "Back"),
    ("print_fronts", "Ön yüzleri yazdır", "Print fronts"),
    ("print", "Yazdır", "Print"),
    ("print_backs", "Arka yüzleri yazdır", "Print backs"),
    (
        "confirm_front",
        "{n} kağıdın ön yüzü <b>{printer}</b> yazıcısına gönderilecek.",
        "The fronts of {n} sheets will be sent to <b>{printer}</b>.",
    ),
    (
        "confirm_single",
        "{n} kağıt <b>{printer}</b> yazıcısına gönderilecek.",
        "{n} sheets will be sent to <b>{printer}</b>.",
    ),
    (
        "confirm_back",
        "{n} kağıdın arka yüzü basılacak. Kağıtları tepsiye koydunuz mu?",
        "The backs of {n} sheets will be printed. Is the paper loaded?",
    ),
    ("printing_fronts", "ön yüz yazdırılıyor", "fronts printing"),
    ("printing_backs", "arka yüz yazdırılıyor", "backs printing"),
    ("printing_single", "kağıt yazdırılıyor", "sheets printing"),
    (
        "wait_all",
        "{n} kağıdın hepsi çıkana kadar bekleyin.",
        "Wait until all {n} sheets are out.",
    ),
    ("flip_title", "Kağıdı geri koyun", "Reload the paper"),
    (
        "flip_1",
        "{n} kağıdı sırasını bozmadan alın.",
        "Take the {n} sheets without changing their order.",
    ),
    (
        "flip_2",
        "Desteyi kitap sayfası çevirir gibi yandan ters çevirin.",
        "Turn the stack over sideways, like turning a book page.",
    ),
    (
        "flip_3",
        "Kağıt tepsisine, boş kağıdı koyduğunuz gibi geri koyun.",
        "Put it back in the paper tray the same way you load blank paper.",
    ),
    (
        "finish_duplex",
        "Bitince desteyi ters çevirin. Sayfalar sırayla olur.",
        "When done, turn the stack over. The pages are in order.",
    ),
    (
        "finish_booklet",
        "Bitince desteyi ters çevirin ve ortadan katlayın.",
        "When done, turn the stack over and fold it in half.",
    ),
    (
        "finish_single",
        "Hepsi çıkınca baskı tamamdır.",
        "Done when all sheets are out.",
    ),
    ("new_document", "Yeni belge", "New document"),
    ("error_title", "Bir sorun oldu", "Something went wrong"),
    ("start_over", "Baştan başla", "Start over"),
    ("err_no_file", "Dosya gelmedi.", "No file received."),
    (
        "err_not_pdf",
        "Bu dosya PDF değil ya da bozuk.",
        "This file is not a valid PDF.",
    ),
    ("err_printer", "Yazıcı bulunamadı.", "Printer not found."),
    (
        "err_range",
        "Sayfa aralığı geçersiz. Belgede {n} sayfa var.",
        "Invalid page range. The document has {n} pages.",
    ),
    ("err_job", "İş bulunamadı.", "Job not found."),
    (
        "err_send",
        "Yazıcıya gönderilemedi: {e}",
        "Could not send to the printer: {e}",
    ),
    (
        "err_no_printers",
        "Yazıcı bulunamadı. CUPS'ta en az bir yazıcı kurulu olmalı.",
        "No printers found. Add a printer in CUPS first.",
    ),
    ("viewer_close", "Kapat", "Close"),
    ("viewer_prev", "Önceki", "Previous"),
    ("viewer_next", "Sonraki", "Next"),
    ("viewer_zoom_in", "Yakınlaştır", "Zoom in"),
    ("viewer_zoom_out", "Uzaklaştır", "Zoom out"),
    ("js_page_list", "Sayfalar: {pages}", "Pages: {pages}"),
    ("js_blank", "boş", "blank"),
    ("js_sheet", "{n}. kağıt", "Sheet {n}"),
    (
        "js_summary",
        "{pages} sayfa · {sheets} kağıt",
        "{pages} pages · {sheets} sheets",
    ),
    (
        "js_copies",
        " × {copies} = {total} kağıt",
        " × {copies} = {total} sheets",
    ),
    ("js_more", "+ {n} kağıt daha", "+ {n} more sheets"),
    (
        "js_not_pdf",
        "Lütfen bir PDF dosyası seçin.",
        "Please choose a PDF file.",
    ),
    ("js_bad_pdf", "PDF okunamadı.", "Could not read the PDF."),
];
