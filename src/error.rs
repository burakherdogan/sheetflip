//! Errors shown to the user as a page in their language.

use crate::{i18n::Lang, ui};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::io;

#[derive(Debug)]
pub enum AppError {
    NoPrinters,
    PrinterNotFound,
    NoFile,
    NotPdf,
    BadRange { page_count: usize },
    JobNotFound,
    Print(String),
    Pdf(lopdf::Error),
    Io(io::Error),
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            AppError::JobNotFound => StatusCode::NOT_FOUND,
            AppError::Print(_) | AppError::Pdf(_) | AppError::Io(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::NoPrinters => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::BAD_REQUEST,
        }
    }

    pub fn message(&self, lang: Lang) -> String {
        match self {
            AppError::NoPrinters => lang.t("err_no_printers").into(),
            AppError::PrinterNotFound => lang.t("err_printer").into(),
            AppError::NoFile => lang.t("err_no_file").into(),
            AppError::NotPdf => lang.t("err_not_pdf").into(),
            AppError::BadRange { page_count } => {
                lang.f("err_range", &[("n", &page_count.to_string())])
            }
            AppError::JobNotFound => lang.t("err_job").into(),
            AppError::Print(e) => lang.f("err_send", &[("e", e)]),
            AppError::Pdf(e) => e.to_string(),
            AppError::Io(e) => e.to_string(),
        }
    }

    pub fn in_lang(self, lang: Lang) -> Failure {
        Failure(lang, self)
    }
}

impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self {
        AppError::Io(e)
    }
}

impl From<lopdf::Error> for AppError {
    fn from(e: lopdf::Error) -> Self {
        AppError::Pdf(e)
    }
}

/// An error together with the language of the page that reports it.
pub struct Failure(Lang, AppError);

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let Failure(lang, error) = self;
        (error.status(), ui::error(lang, &error.message(lang))).into_response()
    }
}

pub type PageResult = Result<axum::response::Html<String>, Failure>;
