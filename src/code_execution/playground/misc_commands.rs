use super::{api::*, util::*};
use crate::{Error, PrefixContext};

use std::borrow::Cow;

/// Run code and detect undefined behavior using Miri
#[poise::command(track_edits, broadcast_typing, explanation_fn = "miri_help")]
pub async fn miri(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let code = &maybe_wrap(&code.code, ResultHandling::Discard);
    let (flags, flag_parse_errors) = parse_flags(flags);

    let mut result: PlayResult = ctx
        .data
        .http
        .post("https://play.rust-lang.org/miri")
        .json(&MiriRequest {
            code,
            edition: flags.edition,
        })
        .send()
        .await?
        .json()
        .await?;

    result.stderr = extract_relevant_lines(
        &result.stderr,
        &["Running `/playground"],
        &["error: aborting"],
    )
    .to_owned();

    send_reply(ctx, result, code, &flags, &flag_parse_errors).await
}

pub fn miri_help() -> String {
    generic_help(GenericHelp {
        command: "miri",
        desc: "Execute this program in the Miri interpreter to detect certain cases of undefined \
        behavior (like out-of-bounds memory access)",
        mode_and_channel: false,
        // Playgrounds sends miri warnings/errors and output in the same field so we can't filter
        // warnings out
        warn: false,
        example_code: "code",
    })
}

/// Expand macros to their raw desugared form
#[poise::command(broadcast_typing, track_edits, explanation_fn = "expand_help")]
pub async fn expand(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let code = maybe_wrap(&code.code, ResultHandling::None);
    let was_fn_main_wrapped = matches!(code, Cow::Owned(_));
    let (flags, flag_parse_errors) = parse_flags(flags);

    let mut result: PlayResult = ctx
        .data
        .http
        .post("https://play.rust-lang.org/macro-expansion")
        .json(&MacroExpansionRequest {
            code: &code,
            edition: flags.edition,
        })
        .send()
        .await?
        .json()
        .await?;

    result.stderr = extract_relevant_lines(
        &result.stderr,
        &["Finished ", "Compiling playground"],
        &["error: aborting"],
    )
    .to_owned();

    if result.success {
        match apply_local_rustfmt(&result.stdout, flags.edition) {
            Ok(PlayResult { success: true, stdout, .. }) => result.stdout = stdout,
            Ok(PlayResult { success: false, stderr, .. }) => log::warn!("Huh, rustfmt failed even though this code successfully passed through macro expansion before: {}", stderr),
            Err(e) => log::warn!("Couldn't run rustfmt: {}", e),
        }
    }
    if was_fn_main_wrapped {
        result.stdout = strip_fn_main_boilerplate_from_formatted(&result.stdout);
    }

    send_reply(ctx, result, &code, &flags, &flag_parse_errors).await
}

pub fn expand_help() -> String {
    generic_help(GenericHelp {
        command: "expand",
        desc: "Expand macros to their raw desugared form",
        mode_and_channel: false,
        warn: false,
        example_code: "code",
    })
}

/// Catch common mistakes using the Clippy linter
#[poise::command(broadcast_typing, track_edits, explanation_fn = "clippy_help")]
pub async fn clippy(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let code = &maybe_wrap(&code.code, ResultHandling::Discard);
    let (flags, flag_parse_errors) = parse_flags(flags);

    let mut result: PlayResult = ctx
        .data
        .http
        .post("https://play.rust-lang.org/clippy")
        .json(&ClippyRequest {
            code,
            edition: flags.edition,
            crate_type: if code.contains("fn main") {
                CrateType::Binary
            } else {
                CrateType::Library
            },
        })
        .send()
        .await?
        .json()
        .await?;

    result.stderr = extract_relevant_lines(
        &result.stderr,
        &["Checking playground", "Running `/playground"],
        &[
            "error: aborting",
            "1 warning emitted",
            "warnings emitted",
            "Finished ",
        ],
    )
    .to_owned();

    send_reply(ctx, result, code, &flags, &flag_parse_errors).await
}

pub fn clippy_help() -> String {
    generic_help(GenericHelp {
        command: "clippy",
        desc: "Catch common mistakes and improve the code using the Clippy linter",
        mode_and_channel: false,
        warn: false,
        example_code: "code",
    })
}

/// Format code using rustfmt
#[poise::command(broadcast_typing, track_edits, explanation_fn = "fmt_help")]
pub async fn fmt(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let code = &maybe_wrap(&code.code, ResultHandling::None);
    let was_fn_main_wrapped = matches!(code, Cow::Owned(_));
    let (flags, flag_parse_errors) = parse_flags(flags);

    let mut result = match apply_local_rustfmt(&code, flags.edition) {
        Ok(x) => x,
        Err(e) => {
            log::warn!("Error while executing local rustfmt: {}", e);
            apply_online_rustfmt(ctx, &code, flags.edition).await?
        }
    };

    if was_fn_main_wrapped {
        result.stdout = strip_fn_main_boilerplate_from_formatted(&result.stdout);
    }

    send_reply(ctx, result, code, &flags, &flag_parse_errors).await
}

pub fn fmt_help() -> String {
    generic_help(GenericHelp {
        command: "fmt",
        desc: "Format code using rustfmt",
        mode_and_channel: false,
        warn: false,
        example_code: "code",
    })
}
