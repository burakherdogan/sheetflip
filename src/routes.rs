//! HTTP routes.

use crate::{
    cups::{self, PrinterSetting, Settings, Side},
    error::{AppError, PageResult},
    i18n::Lang,
    job::{Edge, Job, JobStore},
    pdf::{Mode, layout, parse_pages, test_pdf},
    ui::{esc, page, render, steps},
};
use axum::{
    Form, Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{HeaderMap, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use lopdf::Document;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone)]
pub struct AppState {
    pub jobs: JobStore,
    pub settings: Settings,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/docs", get(docs))
        .route("/lang/{code}", get(set_lang))
        .route("/layout", get(sheet_layout))
        .route("/upload", post(upload))
        .route("/job/{id}", get(summary))
        .route("/job/{id}/front", post(print_front))
        .route("/job/{id}/back", post(print_back))
        .route("/printer-test", get(printer_test).post(start_printer_test))
        .route("/printer-test/{id}", post(finish_printer_test))
        .route(
            "/app.css",
            get(asset("text/css", include_str!("ui/app.css"))),
        )
        .route(
            "/app.js",
            get(asset("text/javascript", include_str!("ui/app.js"))),
        )
        .route(
            "/pdf.mjs",
            get(asset("text/javascript", include_str!("vendor/pdf.mjs"))),
        )
        .route(
            "/pdf.worker.mjs",
            get(asset(
                "text/javascript",
                include_str!("vendor/pdf.worker.mjs"),
            )),
        )
        .layer(DefaultBodyLimit::max(200 * 1024 * 1024))
        .with_state(state)
}

fn asset(content_type: &'static str, body: &'static str) -> impl IntoResponse + Clone {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "max-age=3600"),
        ],
        body,
    )
}

/// `<option>` list of CUPS printers with `preferred` (or the CUPS default) selected.
async fn printer_options(
    lang: Lang,
    preferred: Option<&str>,
) -> Result<String, crate::error::Failure> {
    let printers = cups::printers().await;
    if printers.names.is_empty() {
        return Err(AppError::NoPrinters.in_lang(lang));
    }
    let selected = preferred
        .filter(|p| printers.contains(p))
        .map(str::to_string)
        .or(printers.default);
    Ok(printers
        .names
        .iter()
        .map(|p| {
            let sel = if Some(p) == selected.as_ref() {
                " selected"
            } else {
                ""
            };
            format!("<option{sel}>{}</option>", esc(p))
        })
        .collect())
}

async fn index(State(state): State<AppState>, lang: Lang) -> PageResult {
    let options = printer_options(lang, None).await?;
    let tested: Vec<String> = state.settings.all().into_keys().collect();
    let tested_json = esc(&serde_json::to_string(&tested).unwrap_or_default());
    let body = render(
        include_str!("ui/index.html"),
        lang,
        &[
            ("printer_options", &options),
            ("tested_json", &tested_json),
            ("max_per_side", &crate::job::MAX_PAGES_PER_SIDE.to_string()),
        ],
    );
    Ok(page(lang, lang.t("mode_duplex"), &body))
}

async fn docs(lang: Lang) -> Html<String> {
    let body = match lang {
        Lang::Tr => include_str!("ui/docs_tr.html"),
        Lang::En => include_str!("ui/docs_en.html"),
    };
    page(lang, lang.t("nav_docs"), body)
}

async fn set_lang(Path(code): Path<String>, headers: HeaderMap) -> Response {
    let lang = Lang::parse(&code).unwrap_or(Lang::En);
    // Return to the page the switch was clicked on, unless it was the result of a form post.
    let back = headers
        .get(header::REFERER)
        .and_then(|r| r.to_str().ok())
        .and_then(|r| {
            r.split_once("://")
                .and_then(|(_, rest)| rest.find('/').map(|i| rest[i..].to_string()))
        })
        .filter(|path| {
            !path.ends_with("/front")
                && !path.ends_with("/back")
                && !path.contains("/printer-test/")
        })
        .unwrap_or_else(|| "/".into());
    let cookie = format!(
        "lang={}; Path=/; Max-Age=31536000; SameSite=Lax",
        lang.code()
    );
    ([(header::SET_COOKIE, cookie)], Redirect::to(&back)).into_response()
}

#[derive(Deserialize)]
struct LayoutQuery {
    n: usize,
    mode: Mode,
    #[serde(default)]
    pages: String,
    #[serde(default)]
    per_side: usize,
}

#[derive(Serialize)]
#[serde(untagged)]
enum LayoutResponse {
    Sheets {
        front: Vec<Option<usize>>,
        back: Vec<Option<usize>>,
        columns: usize,
        rows: usize,
    },
    Error {
        error: String,
    },
}

/// Sheet layout for the browser preview, with 0-based page indices and `null` for blank pages.
async fn sheet_layout(lang: Lang, Query(q): Query<LayoutQuery>) -> Json<LayoutResponse> {
    Json(match parse_pages(&q.pages, q.n).filter(|_| q.n > 0) {
        Some(pages) => {
            let per_side = if q.mode == Mode::Booklet {
                2
            } else {
                q.per_side.clamp(1, crate::job::MAX_PAGES_PER_SIDE)
            };
            let (front, back) = layout(q.mode, per_side, &pages);
            let (columns, rows) = crate::pdf::grid(per_side);
            LayoutResponse::Sheets {
                front,
                back,
                columns,
                rows,
            }
        }
        None => LayoutResponse::Error {
            error: AppError::BadRange { page_count: q.n }.message(lang),
        },
    })
}

async fn upload(
    State(state): State<AppState>,
    lang: Lang,
    mut form: Multipart,
) -> Result<Redirect, crate::error::Failure> {
    let fail = |e: AppError| e.in_lang(lang);
    let mut file = None;
    let mut fields = HashMap::new();
    while let Ok(Some(field)) = form.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            file = field.bytes().await.ok();
        } else if let Ok(value) = field.text().await {
            fields.insert(name, value);
        }
    }
    let file = file
        .filter(|b| !b.is_empty())
        .ok_or_else(|| fail(AppError::NoFile))?;
    let page_count = Document::load_mem(&file)
        .map_err(|_| fail(AppError::NotPdf))?
        .get_pages()
        .len();
    let job = Job::from_form(&fields);
    if !cups::printers().await.contains(&job.printer) {
        return Err(fail(AppError::PrinterNotFound));
    }
    if parse_pages(&job.pages, page_count).is_none() {
        return Err(fail(AppError::BadRange { page_count }));
    }
    let id = state
        .jobs
        .create(&file, &job)
        .await
        .map_err(|e| fail(e.into()))?;
    Ok(Redirect::to(&format!("/job/{id}")))
}

fn mode_label(lang: Lang, mode: Mode) -> &'static str {
    lang.t(match mode {
        Mode::Single => "mode_single",
        Mode::Duplex => "mode_duplex",
        Mode::Booklet => "mode_booklet",
    })
}

/// Short page list for one side, e.g. "1, 3, 5, 7 … 15, 17".
fn describe(lang: Lang, order: &[Option<usize>]) -> String {
    let all: Vec<String> = order
        .iter()
        .map(|p| p.map_or_else(|| lang.t("blank").to_string(), |i| (i + 1).to_string()))
        .collect();
    if all.len() <= 8 {
        all.join(", ")
    } else {
        format!(
            "{} … {}",
            all[..4].join(", "),
            all[all.len() - 2..].join(", ")
        )
    }
}

async fn summary(State(state): State<AppState>, lang: Lang, Path(id): Path<String>) -> PageResult {
    let (job, pages) = state
        .jobs
        .load(&id)
        .ok_or_else(|| AppError::JobNotFound.in_lang(lang))?;
    let two_sided = job.mode != Mode::Single;
    let sheets = job.sheets(&pages).to_string();
    let (front, back) = layout(job.mode, job.per_side(), &pages);
    let side_row = |n: usize, key: &str, order: &[Option<usize>]| {
        format!(
            r#"<div class="side-plan"><b>{n}</b><div><strong>{}</strong><small>{}: {}</small></div></div>"#,
            lang.t(key),
            lang.t("pages"),
            describe(lang, order)
        )
    };
    let side_rows = if two_sided {
        side_row(1, "side_front", &front) + &side_row(2, "side_back", &back)
    } else {
        side_row(1, "side_single", &front)
    };
    let copies_note = if job.copies > 1 {
        format!(" · {} × {}", job.copies, lang.t("copies").to_lowercase())
    } else {
        String::new()
    };
    let edge_row = if two_sided {
        let edge = lang.t(if job.edge() == Edge::Short {
            "edge_short"
        } else {
            "edge_long"
        });
        format!("<tr><th>{}</th><td>{edge}</td></tr>", lang.t("flip_edge"))
    } else {
        String::new()
    };
    let printer = esc(&job.printer);
    let confirm = lang.f(
        if two_sided {
            "confirm_front"
        } else {
            "confirm_single"
        },
        &[("n", &sheets), ("printer", &printer)],
    );
    let body = render(
        include_str!("ui/summary.html"),
        lang,
        &[
            ("steps", &steps(lang, two_sided, 0)),
            ("sheet_count", &sheets),
            ("copies_note", &copies_note),
            ("load_paper_text", &lang.f("load_paper", &[("n", &sheets)])),
            ("side_rows", &side_rows),
            ("printer_name", &printer),
            ("mode_label", mode_label(lang, job.mode)),
            (
                "document_pages",
                &lang.f("n_pages", &[("n", &pages.len().to_string())]),
            ),
            (
                "color_label",
                lang.t(if job.grayscale {
                    "grayscale"
                } else {
                    "color_color"
                }),
            ),
            ("quality_label", lang.t(job.quality.label_key())),
            ("edge_row", &edge_row),
            ("per_side", &job.per_side().to_string()),
            (
                "numbers_label",
                lang.t(if job.page_numbers { "on" } else { "off" }),
            ),
            ("id", &id),
            ("confirm", &esc(&confirm)),
            (
                "button",
                lang.t(if two_sided { "print_fronts" } else { "print" }),
            ),
        ],
    );
    Ok(page(lang, lang.t("summary_title"), &body))
}

fn reload_page(lang: Lang, id: &str, sheets: usize) -> Html<String> {
    let n = sheets.to_string();
    let body = render(
        include_str!("ui/front.html"),
        lang,
        &[
            ("steps", &steps(lang, true, 2)),
            ("sheet_count", &n),
            ("wait_text", &lang.f("wait_all", &[("n", &n)])),
            ("flip_1_text", &lang.f("flip_1", &[("n", &n)])),
            ("id", id),
            ("confirm", &esc(&lang.f("confirm_back", &[("n", &n)]))),
        ],
    );
    page(lang, lang.t("flip_title"), &body)
}

fn done_page(
    lang: Lang,
    two_sided: bool,
    sheets: usize,
    status: &str,
    finish: &str,
) -> Html<String> {
    let body = render(
        include_str!("ui/done.html"),
        lang,
        &[
            (
                "steps",
                &steps(lang, two_sided, if two_sided { 4 } else { 2 }),
            ),
            ("sheet_count", &sheets.to_string()),
            ("status", lang.t(status)),
            ("finish", lang.t(finish)),
        ],
    );
    page(lang, lang.t("step_done"), &body)
}

async fn print_front(
    State(state): State<AppState>,
    lang: Lang,
    Path(id): Path<String>,
) -> PageResult {
    let (job, sheets) = cups::print_side(&state.jobs, &state.settings, &id, Side::Front)
        .await
        .map_err(|e| e.in_lang(lang))?;
    if job.mode == Mode::Single {
        state.jobs.remove(&id).await;
        return Ok(done_page(
            lang,
            false,
            sheets,
            "printing_single",
            "finish_single",
        ));
    }
    Ok(reload_page(lang, &id, sheets))
}

async fn print_back(
    State(state): State<AppState>,
    lang: Lang,
    Path(id): Path<String>,
) -> PageResult {
    let (job, sheets) = cups::print_side(&state.jobs, &state.settings, &id, Side::Back)
        .await
        .map_err(|e| e.in_lang(lang))?;
    if job.test {
        let body = render(
            include_str!("ui/test_check.html"),
            lang,
            &[("steps", &steps(lang, true, 3)), ("id", &id)],
        );
        return Ok(page(lang, lang.t("check_title"), &body));
    }
    state.jobs.remove(&id).await;
    let finish = if job.mode == Mode::Booklet {
        "finish_booklet"
    } else {
        "finish_duplex"
    };
    Ok(done_page(lang, true, sheets, "printing_backs", finish))
}

#[derive(Deserialize)]
struct PrinterQuery {
    printer: Option<String>,
}

async fn printer_test(
    State(state): State<AppState>,
    lang: Lang,
    Query(q): Query<PrinterQuery>,
) -> PageResult {
    let options = printer_options(lang, q.printer.as_deref()).await?;
    let tested: Vec<String> = state.settings.all().into_keys().collect();
    let tested_text = if tested.is_empty() {
        String::new()
    } else {
        esc(&lang.f("tested_list", &[("list", &tested.join(", "))]))
    };
    let body = render(
        include_str!("ui/printer_test.html"),
        lang,
        &[("printer_options", &options), ("tested_text", &tested_text)],
    );
    Ok(page(lang, lang.t("test_title"), &body))
}

#[derive(Deserialize)]
struct PrinterForm {
    printer: String,
}

async fn start_printer_test(
    State(state): State<AppState>,
    lang: Lang,
    Form(f): Form<PrinterForm>,
) -> PageResult {
    let fail = |e: AppError| e.in_lang(lang);
    if !cups::printers().await.contains(&f.printer) {
        return Err(fail(AppError::PrinterNotFound));
    }
    let pdf = test_pdf(lang.t("test_top"), lang.t("test_back"));
    let id = state
        .jobs
        .create(&pdf, &Job::printer_test(f.printer))
        .await
        .map_err(|e| fail(e.into()))?;
    let (_, sheets) = cups::print_side(&state.jobs, &state.settings, &id, Side::Front)
        .await
        .map_err(fail)?;
    Ok(reload_page(lang, &id, sheets))
}

#[derive(Deserialize)]
struct TestAnswers {
    /// The number found on the back of sheet 1: "2", "4" or "none".
    back: String,
    /// Direction of the arrow on the back: "up" or "down".
    arrow: String,
}

async fn finish_printer_test(
    State(state): State<AppState>,
    lang: Lang,
    Path(id): Path<String>,
    Form(a): Form<TestAnswers>,
) -> PageResult {
    let (job, _) = state
        .jobs
        .load(&id)
        .filter(|(job, _)| job.test)
        .ok_or_else(|| AppError::JobNotFound.in_lang(lang))?;
    state.jobs.remove(&id).await;
    let retry = format!("/printer-test?printer={}", esc(&job.printer));
    if a.back == "none" {
        return Ok(message_page(
            lang,
            lang.t("error_title"),
            lang.t("test_blank_back"),
            (lang.t("retry"), &retry),
            (lang.t("back_link"), "/"),
        ));
    }
    let setting = PrinterSetting {
        reverse: a.back == "4",
        rotate: a.arrow == "down",
    };
    state
        .settings
        .save(&job.printer, setting)
        .map_err(|e| AppError::from(e).in_lang(lang))?;
    let text = lang.f("test_saved", &[("printer", &esc(&job.printer))]);
    Ok(message_page(
        lang,
        lang.t("test_saved_title"),
        &text,
        (lang.t("new_document"), "/"),
        (lang.t("retry"), &retry),
    ))
}

fn message_page(
    lang: Lang,
    heading: &str,
    message: &str,
    primary: (&str, &str),
    secondary: (&str, &str),
) -> Html<String> {
    let body = render(
        include_str!("ui/message.html"),
        lang,
        &[
            ("heading", heading),
            ("message", message),
            ("primary", primary.0),
            ("primary_href", primary.1),
            ("secondary", secondary.0),
            ("secondary_href", secondary.1),
        ],
    );
    page(lang, heading, &body)
}
