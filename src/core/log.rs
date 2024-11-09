use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{
    fmt::{
        self,
        format::{FmtSpan, Writer},
        FmtContext, FormatEvent, FormatFields,
    },
    registry::LookupSpan,
};

use crate::cli::Args;

use super::color::{Color, ColorExt};

pub struct CustomFormatter;

impl<S, N> FormatEvent<S, N> for CustomFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        match *event.metadata().level() {
            Level::TRACE => write!(writer, "{} ", "[TRACE]".color(Color::BrightMagenta)),
            Level::DEBUG => write!(writer, "{} ", "[DEBUG]".color(Color::BrightBlue)),
            Level::INFO => write!(writer, ""),
            Level::WARN => write!(writer, "{} ", "[WARN]".color(Color::BrightYellow)),
            Level::ERROR => write!(writer, "{} ", "[ERROR]".color(Color::BrightRed)),
        }?;

        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}

pub struct CleanJsonFormatter;

impl<S, N> FormatEvent<S, N> for CleanJsonFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let mut fields_output = String::new();

        let temp_writer = Writer::new(&mut fields_output);
        ctx.field_format().format_fields(temp_writer, event)?;
        let clean_fields = strip_ansi_escapes::strip_str(&fields_output);

        let json = serde_json::json!({
            "level": event.metadata().level().to_string(),
            "fields": clean_fields,
            "target": event.metadata().target(),
        });

        writeln!(writer, "{}", json)
    }
}

pub fn setup_logging(args: &Args) {
    let filter_level = if args.quiet {
        Level::ERROR
    } else if args.verbose >= 2 {
        Level::TRACE
    } else if args.verbose == 1 {
        Level::DEBUG
    } else {
        Level::INFO
    };

    let builder = fmt::Subscriber::builder()
        .with_env_filter(format!("soar_cli={}", filter_level))
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .with_span_events(FmtSpan::NONE)
        .with_writer(std::io::stderr)
        .compact()
        .without_time();

    let subscriber: Box<dyn Subscriber + Send + Sync> = if args.json {
        Box::new(builder.event_format(CleanJsonFormatter).finish())
    } else {
        Box::new(builder.event_format(CustomFormatter).finish())
    };

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
}
