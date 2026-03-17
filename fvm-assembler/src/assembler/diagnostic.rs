use ariadne::{sources, Color, Label, Report, ReportKind};

use crate::error::AssemblerError;

pub fn render_error(error: &AssemblerError) -> String {
    let Some(files) = error.files() else {
        return error.to_string();
    };

    let Some(loc) = error.loc() else {
        return error.to_string();
    };

    let source_id = files.path(loc.file).display().to_string();
    let span = (source_id.clone(), loc.span.clone());
    let report = Report::build(ReportKind::Error, span.clone())
        .with_message(error.to_string())
        .with_label(
            Label::new(span)
                .with_message(error.to_string())
                .with_color(Color::Red),
        )
        .finish();

    let mut out = Vec::new();
    let cache = sources(
        files
            .iter()
            .map(|(_, path, source)| (path.display().to_string(), source.to_string()))
            .collect::<Vec<_>>(),
    );
    if report.write(cache, &mut out).is_err() {
        return error.to_string();
    }

    String::from_utf8_lossy(&out).into_owned()
}