//! Numbers written for the active language: decimal and grouping separators from CLDR data through
//! `icu_decimal`. `1,234.5` in English, `1234,5` and `12.345,5` in Spanish, and so on. Unit symbols
//! stay as the caller writes them.
//!
//! The rounding is Rust's own (`{:.N}`), so the digits are exactly the ones the UI showed
//! before; the formatted text is then handed to ICU only to place the separators. The formatter
//! of the active language is built once and cached under its language code, so the helpers are
//! cheap enough for a render path. Plain Rust, no `gpui`.
//!
//! Only for numbers shown to the user. Logs, code snippets, the run history JSON and the text
//! of the number inputs the user types in stay in plain `to_string()`/`{:.N}` form.

use std::cell::RefCell;
use std::rc::Rc;

use icu_decimal::input::{Decimal, SignDisplay};
use icu_decimal::options::DecimalFormatterOptions;
use icu_decimal::{DecimalFormatter, DecimalFormatterPreferences};
use icu_locale_core::Locale;

use super::locale::{Language, match_language};

/// The formatter of the last language asked for, keyed by its locale code.
type Cached = Option<(&'static str, Rc<DecimalFormatter>)>;

thread_local! {
    // `DecimalFormatter` is not `Send`, so the cache lives per thread. The UI thread is the only
    // one that renders numbers.
    static FORMATTER: RefCell<Cached> = const { RefCell::new(None) };
}

/// The language the UI speaks right now (`rust_i18n::locale()`), English if it is not one of
/// ours.
fn active_language() -> Language {
    match_language(&rust_i18n::locale()).unwrap_or(Language::English)
}

/// The formatter for `language`, built on the first call for it and reused until another
/// language is asked for. `None` if ICU has no data for it, which compiled data rules out.
fn formatter(language: Language) -> Option<Rc<DecimalFormatter>> {
    FORMATTER.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((code, formatter)) = cache.as_ref()
            && *code == language.code()
        {
            return Some(Rc::clone(formatter));
        }
        let locale = Locale::try_from_str(language.code()).ok()?;
        let built = DecimalFormatter::try_new(
            DecimalFormatterPreferences::from(&locale),
            DecimalFormatterOptions::default(),
        )
        .ok()?;
        let built = Rc::new(built);
        *cache = Some((language.code(), Rc::clone(&built)));
        Some(built)
    })
}

/// Formats `plain` (an ASCII number such as `-1234.50`) with `language`'s separators, `plain`
/// itself if it does not parse (`NaN`, infinity) or no formatter exists.
fn localize(plain: String, language: Language, sign: SignDisplay) -> String {
    let Ok(decimal) = Decimal::try_from_str(&plain) else {
        return plain;
    };
    match formatter(language) {
        Some(formatter) => formatter.format_to_string(&decimal.with_sign_display(sign)),
        None => plain,
    }
}

/// `value` with the active language's grouping, for example `17,304` or `17.304`.
pub fn format_integer(value: impl Into<u64>) -> String {
    format_integer_in(value.into(), active_language())
}

/// `value` with exactly `fraction_digits` decimals, rounded as `{:.N}` does and written with
/// the active language's separators, for example `1.5` or `1,5`.
pub fn format_decimal(value: f64, fraction_digits: usize) -> String {
    format_decimal_in(value, fraction_digits, active_language())
}

/// [`format_integer`] in an explicit `language`, so tests need not touch the process wide one.
fn format_integer_in(value: u64, language: Language) -> String {
    localize(value.to_string(), language, SignDisplay::Auto)
}

/// [`format_decimal`] in an explicit `language`.
fn format_decimal_in(value: f64, fraction_digits: usize, language: Language) -> String {
    localize(
        format!("{value:.fraction_digits$}"),
        language,
        SignDisplay::Auto,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn integers_group_per_language() {
        let expected = [
            (
                Language::English,
                ["0", "999", "1,234", "17,304", "1,234,567"],
            ),
            (
                Language::Spanish,
                ["0", "999", "1234", "17.304", "1.234.567"],
            ),
            (
                Language::Galician,
                ["0", "999", "1.234", "17.304", "1.234.567"],
            ),
            (
                Language::Italian,
                ["0", "999", "1234", "17.304", "1.234.567"],
            ),
        ];
        for (language, texts) in expected {
            let values = [0u64, 999, 1234, 17_304, 1_234_567];
            for (value, text) in values.into_iter().zip(texts) {
                assert_eq!(format_integer_in(value, language), text, "{language:?}");
            }
        }
    }

    #[test]
    fn decimals_keep_the_requested_fraction_digits() {
        assert_eq!(format_decimal_in(1.5, 1, Language::English), "1.5");
        assert_eq!(format_decimal_in(1.5, 1, Language::Spanish), "1,5");
        assert_eq!(format_decimal_in(1.5, 1, Language::Galician), "1,5");
        assert_eq!(format_decimal_in(1.5, 1, Language::Italian), "1,5");
        assert_eq!(format_decimal_in(2.0, 1, Language::Spanish), "2,0");
        assert_eq!(format_decimal_in(1.2, 2, Language::Italian), "1,20");
        assert_eq!(format_decimal_in(1234.5, 1, Language::English), "1,234.5");
        assert_eq!(format_decimal_in(1234.5, 1, Language::Spanish), "1234,5");
        assert_eq!(format_decimal_in(12345.5, 1, Language::Spanish), "12.345,5");
        assert_eq!(format_decimal_in(12345.5, 1, Language::Italian), "12.345,5");
        assert_eq!(format_decimal_in(1234.5, 1, Language::Galician), "1.234,5");
    }

    #[test]
    fn rounding_matches_the_standard_formatter() {
        for value in [0.04, 0.05, 0.25, 2.675, 99.95, 3.3, 1229.0 / 1024.0] {
            let plain = format!("{value:.1}");
            assert_eq!(format_decimal_in(value, 1, Language::English), plain);
        }
    }

    #[test]
    fn negative_numbers_keep_their_sign() {
        assert_eq!(format_decimal_in(-2.34, 1, Language::English), "-2.3");
        assert_eq!(format_decimal_in(-2.34, 1, Language::Spanish), "-2,3");
        assert_eq!(
            format_decimal_in(-12345.6, 1, Language::Italian),
            "-12.345,6"
        );
    }

    #[test]
    fn non_finite_values_fall_back_to_the_plain_text() {
        assert_eq!(format_decimal_in(f64::NAN, 1, Language::Spanish), "NaN");
        assert_eq!(
            format_decimal_in(f64::INFINITY, 1, Language::Spanish),
            "inf"
        );
    }
}
