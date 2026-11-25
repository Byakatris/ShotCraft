/// Minimal zero-dependency JSON parser for GitHub Releases API responses.
#[derive(Debug)]
pub(crate) enum JsonVal {
    Null,
    Bool(bool),
    U64(u64),
    OtherNum,
    Str(String),
    Arr(Vec<JsonVal>),
    Obj(Vec<(String, JsonVal)>),
}

impl JsonVal {
    pub(crate) fn field(&self, key: &str) -> Option<&JsonVal> {
        let JsonVal::Obj(entries) = self else {
            return None;
        };
        entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            JsonVal::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub(crate) fn as_bool(&self) -> Option<bool> {
        match self {
            JsonVal::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub(crate) fn as_u64(&self) -> Option<u64> {
        match self {
            JsonVal::U64(n) => Some(*n),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[JsonVal]> {
        match self {
            JsonVal::Arr(v) => Some(v.as_slice()),
            _ => None,
        }
    }
}

pub(crate) struct JsonParser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            src: input.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        self.skip_ws();
        match self.peek() {
            Some(b) if b == byte => {
                self.pos += 1;
                Ok(())
            }
            other => Err(format!(
                "JSON parse error at {}: expected '{}', found {:?}",
                self.pos,
                byte as char,
                other.map(|c| c as char)
            )),
        }
    }

    pub(crate) fn parse_root(&mut self) -> Result<JsonVal, String> {
        let val = self.parse_value(0)?;
        self.skip_ws();
        if self.pos < self.src.len() {
            return Err(format!("Trailing JSON bytes at {}", self.pos));
        }
        Ok(val)
    }

    fn parse_value(&mut self, depth: usize) -> Result<JsonVal, String> {
        if depth > 64 {
            return Err("JSON nesting depth exceeded".to_string());
        }
        self.skip_ws();
        let b = self
            .peek()
            .ok_or_else(|| "Unexpected end of JSON".to_string())?;
        match b {
            b'"' => Ok(JsonVal::Str(self.parse_string()?)),
            b'{' => self.parse_object(depth + 1),
            b'[' => self.parse_array(depth + 1),
            b't' => self.consume_literal(b"true", JsonVal::Bool(true)),
            b'f' => self.consume_literal(b"false", JsonVal::Bool(false)),
            b'n' => self.consume_literal(b"null", JsonVal::Null),
            b'-' | b'0'..=b'9' => self.parse_number(),
            other => Err(format!("Unexpected JSON byte '{}' at {}", other as char, self.pos)),
        }
    }

    fn consume_literal(&mut self, lit: &[u8], val: JsonVal) -> Result<JsonVal, String> {
        if self.src.get(self.pos..self.pos + lit.len()) == Some(lit) {
            self.pos += lit.len();
            Ok(val)
        } else {
            Err(format!("Invalid JSON literal at {}", self.pos))
        }
    }

    fn parse_number(&mut self) -> Result<JsonVal, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(format!("Invalid JSON number at {start}")),
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(format!("Invalid JSON fraction at {}", self.pos));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(format!("Invalid JSON exponent at {}", self.pos));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).map_err(|e| e.to_string())?;
        if !is_float && !s.starts_with('-') {
            let n = s
                .parse::<u64>()
                .map_err(|e| format!("Invalid JSON u64 '{s}': {e}"))?;
            Ok(JsonVal::U64(n))
        } else {
            let f = s
                .parse::<f64>()
                .map_err(|e| format!("Invalid JSON float '{s}': {e}"))?;
            if !f.is_finite() {
                return Err(format!("Non-finite JSON number '{s}'"));
            }
            Ok(JsonVal::OtherNum)
        }
    }

    fn read_hex_u16(&mut self) -> Result<u16, String> {
        let hex = self
            .src
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| "Truncated JSON unicode escape".to_string())?;
        self.pos += 4;
        u16::from_str_radix(
            std::str::from_utf8(hex).map_err(|e| e.to_string())?,
            16,
        )
        .map_err(|e| e.to_string())
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        while self.pos < self.src.len() {
            let b = self.src[self.pos];
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = *self
                        .src
                        .get(self.pos)
                        .ok_or_else(|| "Unterminated JSON escape".to_string())?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let code = self.read_hex_u16()?;
                            if (0xD800..=0xDBFF).contains(&code) {
                                if self.src.get(self.pos..self.pos + 2) == Some(b"\\u") {
                                    self.pos += 2;
                                    let low = self.read_hex_u16()?;
                                    if (0xDC00..=0xDFFF).contains(&low) {
                                        let combined = 0x10000
                                            + ((u32::from(code) - 0xD800) << 10)
                                            + (u32::from(low) - 0xDC00);
                                        if let Some(ch) = char::from_u32(combined) {
                                            out.push(ch);
                                        }
                                    }
                                }
                            } else if !(0xDC00..=0xDFFF).contains(&code)
                                && let Some(ch) = char::from_u32(u32::from(code))
                            {
                                out.push(ch);
                            }
                        }
                        other => out.push(other as char),
                    }
                }
                _ if b < 0x80 => out.push(b as char),
                first => {
                    let start = self.pos - 1;
                    let width = if first & 0b1110_0000 == 0b1100_0000 {
                        2
                    } else if first & 0b1111_0000 == 0b1110_0000 {
                        3
                    } else {
                        4
                    };
                    let end = (start + width).min(self.src.len());
                    let s = std::str::from_utf8(&self.src[start..end]).map_err(|e| e.to_string())?;
                    out.push_str(s);
                    self.pos = end;
                }
            }
        }
        Err("Unterminated JSON string".to_string())
    }

    fn parse_array(&mut self, depth: usize) -> Result<JsonVal, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonVal::Arr(items));
        }
        loop {
            items.push(self.parse_value(depth)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonVal::Arr(items));
                }
                _ => return Err(format!("Expected ',' or ']' at {}", self.pos)),
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<JsonVal, String> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonVal::Obj(entries));
        }
        loop {
            let key = self.parse_string()?;
            self.expect(b':')?;
            let val = self.parse_value(depth)?;
            entries.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonVal::Obj(entries));
                }
                _ => return Err(format!("Expected ',' or '}}' at {}", self.pos)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_parser_surrogates_numbers_and_validation() {
        let parsed = JsonParser::new(r#"{"emoji":"\uD83D\uDE80","flag":true,"n":42,"neg":-5,"flt":3.5}"#)
            .parse_root()
            .unwrap();
        assert_eq!(parsed.field("emoji").and_then(JsonVal::as_str), Some("🚀"));
        assert_eq!(parsed.field("flag").and_then(JsonVal::as_bool), Some(true));
        assert_eq!(parsed.field("n").and_then(JsonVal::as_u64), Some(42));
        assert_eq!(parsed.field("neg").and_then(JsonVal::as_u64), None);
        assert_eq!(parsed.field("flt").and_then(JsonVal::as_u64), None);

        assert!(JsonParser::new(r#"{"a":1} trailing"#).parse_root().is_err());
        assert!(JsonParser::new(r#"{"a":1--2}"#).parse_root().is_err());
        assert!(JsonParser::new(r#"{"unterminated""#).parse_root().is_err());
    }
}
