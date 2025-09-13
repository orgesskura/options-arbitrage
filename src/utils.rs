use chrono::{NaiveDate, ParseError};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInstrument {
    pub underlying: String,
    pub expiry_date: NaiveDate,
    pub strike: u32,
    pub option_type: OptionType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionType {
    Call,
    Put,
}

#[derive(Debug, thiserror::Error)]
pub enum InstrumentParseError {
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error("Date parse error: {0}")]
    DateParseError(#[from] ParseError),
    #[error("Invalid strike price: {0}")]
    InvalidStrike(String),
    #[error("Invalid option type: {0}")]
    InvalidOptionType(String),
    #[error("Insufficient components in symbol")]
    InsufficientComponents,
}

pub struct InstrumentValidator;

impl InstrumentValidator {
    pub fn are_same_instrument(
        okex_symbol: &str,
        deribit_symbol: &str,
    ) -> Result<bool, InstrumentParseError> {
        let okex_parsed = Self::parse_okex_symbol(okex_symbol)?;
        let deribit_parsed = Self::parse_deribit_symbol(deribit_symbol)?;

        Ok(okex_parsed == deribit_parsed)
    }

    fn parse_okex_symbol(symbol: &str) -> Result<ParsedInstrument, InstrumentParseError> {
        let parts: Vec<&str> = symbol.split('-').collect();
        if parts.len() < 5 {
            return Err(InstrumentParseError::InsufficientComponents);
        }

        let base = parts[0];
        let date_str = parts[2];
        let strike_str = parts[3];
        let option_type_str = parts[4];

        let underlying = base.to_uppercase();
        let expiry_date = Self::parse_okex_date(date_str)?;
        let strike: u32 = strike_str
            .parse()
            .map_err(|_| InstrumentParseError::InvalidStrike(strike_str.to_string()))?;
        let option_type = match option_type_str.to_uppercase().as_str() {
            "C" => OptionType::Call,
            "P" => OptionType::Put,
            _ => {
                return Err(InstrumentParseError::InvalidOptionType(
                    option_type_str.to_string(),
                ));
            }
        };

        Ok(ParsedInstrument {
            underlying,
            expiry_date,
            strike,
            option_type,
        })
    }

    fn parse_deribit_symbol(symbol: &str) -> Result<ParsedInstrument, InstrumentParseError> {
        let parts: Vec<&str> = symbol.split('-').collect();
        if parts.len() < 4 {
            return Err(InstrumentParseError::InsufficientComponents);
        }

        let underlying = parts[0].to_uppercase();
        let date_str = parts[1];
        let strike_str = parts[2];
        let option_type_str = parts[3];

        let expiry_date = Self::parse_deribit_date(date_str)?;
        let strike: u32 = strike_str
            .parse()
            .map_err(|_| InstrumentParseError::InvalidStrike(strike_str.to_string()))?;
        let option_type = match option_type_str.to_uppercase().as_str() {
            "C" => OptionType::Call,
            "P" => OptionType::Put,
            _ => {
                return Err(InstrumentParseError::InvalidOptionType(
                    option_type_str.to_string(),
                ));
            }
        };

        Ok(ParsedInstrument {
            underlying,
            expiry_date,
            strike,
            option_type,
        })
    }

    fn parse_okex_date(date_str: &str) -> Result<NaiveDate, InstrumentParseError> {
        if date_str.len() != 6 {
            return Err(InstrumentParseError::InvalidFormat(format!(
                "Expected 6-digit date, got: {date_str}",
            )));
        }

        let year_str = &date_str[0..2];
        let month_str = &date_str[2..4];
        let day_str = &date_str[4..6];

        let year: i32 = year_str.parse().map_err(|_| {
            InstrumentParseError::InvalidFormat(format!("Invalid year: {year_str}"))
        })?;
        let month: u32 = month_str.parse().map_err(|_| {
            InstrumentParseError::InvalidFormat(format!("Invalid month: {month_str}"))
        })?;
        let day: u32 = day_str
            .parse()
            .map_err(|_| InstrumentParseError::InvalidFormat(format!("Invalid day: {day_str}")))?;

        // Convert 2-digit year to 4-digit (assuming 20XX for years 00-99)
        let full_year = if (0..=99).contains(&year) {
            2000 + year
        } else {
            return Err(InstrumentParseError::InvalidFormat(format!(
                "Invalid year: {year}",
            )));
        };

        NaiveDate::from_ymd_opt(full_year, month, day).ok_or_else(|| {
            InstrumentParseError::InvalidFormat(format!(
                "Invalid date: {full_year}-{month:02}-{day:02}",
            ))
        })
    }

    fn parse_deribit_date(date_str: &str) -> Result<NaiveDate, InstrumentParseError> {
        if date_str.len() < 7 {
            return Err(InstrumentParseError::InvalidFormat(format!(
                "Expected format DDMMMYY, got: {date_str}",
            )));
        }

        let day_str = &date_str[0..2];
        let month_str = &date_str[2..5];
        let year_str = &date_str[5..7];

        let day: u32 = day_str
            .parse()
            .map_err(|_| InstrumentParseError::InvalidFormat(format!("Invalid day: {day_str}")))?;

        let year: i32 = year_str.parse().map_err(|_| {
            InstrumentParseError::InvalidFormat(format!("Invalid year: {year_str}"))
        })?;

        let month_map: HashMap<&str, u32> = [
            ("JAN", 1),
            ("FEB", 2),
            ("MAR", 3),
            ("APR", 4),
            ("MAY", 5),
            ("JUN", 6),
            ("JUL", 7),
            ("AUG", 8),
            ("SEP", 9),
            ("OCT", 10),
            ("NOV", 11),
            ("DEC", 12),
        ]
        .iter()
        .cloned()
        .collect();

        let month = *month_map
            .get(month_str.to_uppercase().as_str())
            .ok_or_else(|| {
                InstrumentParseError::InvalidFormat(format!("Invalid month: {month_str}"))
            })?;

        // Convert 2-digit year to 4-digit (assuming 20XX for years 00-99)
        let full_year = if (0..=99).contains(&year) {
            2000 + year
        } else {
            return Err(InstrumentParseError::InvalidFormat(format!(
                "Invalid year: {year}",
            )));
        };

        NaiveDate::from_ymd_opt(full_year, month, day).ok_or_else(|| {
            InstrumentParseError::InvalidFormat(format!(
                "Invalid date: {full_year}-{month:02}-{day:02}",
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, NaiveDate};

    #[test]
    fn test_same_instruments() {
        let okex = "BTC-USD-240427-56000-C";
        let deribit = "BTC-27APR24-56000-C";

        assert!(InstrumentValidator::are_same_instrument(okex, deribit).unwrap());
    }

    #[test]
    fn test_different_strikes() {
        let okex = "BTC-USD-240427-56000-C";
        let deribit = "BTC-27APR24-60000-C";

        assert!(!InstrumentValidator::are_same_instrument(okex, deribit).unwrap());
    }

    #[test]
    fn test_different_expiry() {
        let okex = "BTC-USD-240427-56000-C";
        let deribit = "BTC-28APR24-56000-C";

        assert!(!InstrumentValidator::are_same_instrument(okex, deribit).unwrap());
    }

    #[test]
    fn test_different_option_types() {
        let okex = "BTC-USD-240427-56000-C";
        let deribit = "BTC-27APR24-56000-P";

        assert!(!InstrumentValidator::are_same_instrument(okex, deribit).unwrap());
    }

    #[test]
    fn test_put_options() {
        let okex = "BTC-USD-251031-140000-P";
        let deribit = "BTC-31OCT25-140000-P";

        assert!(InstrumentValidator::are_same_instrument(okex, deribit).unwrap());
    }

    #[test]
    fn test_date_parsing() {
        let date1 = InstrumentValidator::parse_okex_date("240427").unwrap();
        let expected1 = NaiveDate::from_ymd_opt(2024, 4, 27).unwrap();
        assert_eq!(date1, expected1);

        let date2 = InstrumentValidator::parse_deribit_date("27APR24").unwrap();
        let expected2 = NaiveDate::from_ymd_opt(2024, 4, 27).unwrap();
        assert_eq!(date2, expected2);
    }

    #[test]
    fn test_invalid_formats() {
        assert!(InstrumentValidator::parse_okex_symbol("BTC-USD-240427").is_err());
        assert!(InstrumentValidator::parse_deribit_symbol("BTC-27APR24").is_err());

        assert!(InstrumentValidator::parse_okex_date("24042").is_err()); // Too short
        assert!(InstrumentValidator::parse_deribit_date("27XYZ24").is_err()); // Invalid month

        // Test invalid strike
        assert!(InstrumentValidator::parse_okex_symbol("BTC-USD-240427-ABC-C").is_err());

        // Test invalid option type
        assert!(InstrumentValidator::parse_okex_symbol("BTC-USD-240427-56000-X").is_err());
    }

    #[test]
    fn test_parsed_instrument_components() {
        let okex_parsed = InstrumentValidator::parse_okex_symbol("BTC-USD-240427-56000-C").unwrap();

        assert_eq!(okex_parsed.underlying, "BTC");
        assert_eq!(
            okex_parsed.expiry_date,
            NaiveDate::from_ymd_opt(2024, 4, 27).unwrap()
        );
        assert_eq!(okex_parsed.strike, 56000);
        assert_eq!(okex_parsed.option_type, OptionType::Call);

        let deribit_parsed =
            InstrumentValidator::parse_deribit_symbol("BTC-27APR24-56000-C").unwrap();

        assert_eq!(deribit_parsed.underlying, "BTC");
        assert_eq!(
            deribit_parsed.expiry_date,
            NaiveDate::from_ymd_opt(2024, 4, 27).unwrap()
        );
        assert_eq!(deribit_parsed.strike, 56000);
        assert_eq!(deribit_parsed.option_type, OptionType::Call);

        assert_eq!(okex_parsed, deribit_parsed);
    }

    #[test]
    fn test_edge_case_dates() {
        // Test year boundary cases
        let okex_99 = InstrumentValidator::parse_okex_date("991231").unwrap();
        assert_eq!(okex_99.year(), 2099);

        let okex_00 = InstrumentValidator::parse_okex_date("000101").unwrap();
        assert_eq!(okex_00.year(), 2000);

        // Test all months for Deribit
        let months = [
            ("01JAN24", 1),
            ("15FEB24", 2),
            ("31MAR24", 3),
            ("30APR24", 4),
            ("31MAY24", 5),
            ("30JUN24", 6),
            ("31JUL24", 7),
            ("31AUG24", 8),
            ("30SEP24", 9),
            ("31OCT24", 10),
            ("30NOV24", 11),
            ("31DEC24", 12),
        ];

        for (date_str, expected_month) in months {
            let parsed = InstrumentValidator::parse_deribit_date(date_str).unwrap();
            assert_eq!(parsed.month(), expected_month);
        }
    }
}
