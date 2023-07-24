// sadly ranges don't optmimzie well
#![allow(clippy::manual_range_contains)]

pub mod chars;
mod config;
#[cfg(test)]
mod debug;
mod exact;
mod fuzzy_greedy;
mod fuzzy_optimal;
mod matrix;
mod prefilter;
mod score;
mod utf32_str;

#[cfg(test)]
mod tests;

pub use crate::config::MatcherConfig;
pub use crate::utf32_str::Utf32Str;

use crate::chars::AsciiChar;
use crate::matrix::MatrixSlab;

pub struct Matcher {
    pub config: MatcherConfig,
    slab: MatrixSlab,
}

impl Matcher {
    pub fn new(config: MatcherConfig) -> Self {
        Self {
            config,
            slab: MatrixSlab::new(),
        }
    }

    pub fn fuzzy_match(&mut self, haystack: Utf32Str<'_>, needle: Utf32Str<'_>) -> Option<u16> {
        assert!(haystack.len() <= u32::MAX as usize);
        self.fuzzy_matcher_impl::<false>(haystack, needle, &mut Vec::new())
    }

    pub fn fuzzy_indices(
        &mut self,
        haystack: Utf32Str<'_>,
        needle: Utf32Str<'_>,
        indidies: &mut Vec<u32>,
    ) -> Option<u16> {
        assert!(haystack.len() <= u32::MAX as usize);
        self.fuzzy_matcher_impl::<true>(haystack, needle, indidies)
    }

    fn fuzzy_matcher_impl<const INDICES: bool>(
        &mut self,
        haystack: Utf32Str<'_>,
        needle_: Utf32Str<'_>,
        indidies: &mut Vec<u32>,
    ) -> Option<u16> {
        if needle_.len() > haystack.len() || needle_.is_empty() {
            return None;
        }
        // if needle_.len() == haystack.len() {
        //     return self.exact_match();
        // }
        assert!(
            haystack.len() <= u32::MAX as usize,
            "fuzzy matching is only support for up to 2^32-1 codepoints"
        );
        match (haystack, needle_) {
            (Utf32Str::Ascii(haystack), Utf32Str::Ascii(needle)) => {
                if let &[needle] = needle {
                    return self.substring_match_1_ascii::<INDICES>(haystack, needle, indidies);
                }
                let (start, greedy_end, end) = self.prefilter_ascii(haystack, needle, false)?;
                self.fuzzy_match_optimal::<INDICES, AsciiChar, AsciiChar>(
                    AsciiChar::cast(haystack),
                    AsciiChar::cast(needle),
                    start,
                    greedy_end,
                    end,
                    indidies,
                )
            }
            (Utf32Str::Ascii(_), Utf32Str::Unicode(_)) => {
                // a purely ascii haystack can never be transformed to match
                // a needle that contains non-ascii chars since we don't allow gaps
                None
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Ascii(needle)) => {
                if let &[needle] = needle {
                    let (start, _) = self.prefilter_non_ascii(haystack, needle_, true)?;
                    let res = self.substring_match_1_non_ascii::<INDICES>(
                        haystack,
                        needle as char,
                        start,
                        indidies,
                    );
                    return Some(res);
                }
                let (start, end) = self.prefilter_non_ascii(haystack, needle_, false)?;
                self.fuzzy_match_optimal::<INDICES, char, AsciiChar>(
                    haystack,
                    AsciiChar::cast(needle),
                    start,
                    start + 1,
                    end,
                    indidies,
                )
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Unicode(needle)) => {
                if let &[needle] = needle {
                    let (start, _) = self.prefilter_non_ascii(haystack, needle_, true)?;
                    let res = self
                        .substring_match_1_non_ascii::<INDICES>(haystack, needle, start, indidies);
                    return Some(res);
                }
                let (start, end) = self.prefilter_non_ascii(haystack, needle_, false)?;
                self.fuzzy_match_optimal::<INDICES, char, char>(
                    haystack,
                    needle,
                    start,
                    start + 1,
                    end,
                    indidies,
                )
            }
        }
    }
    pub fn fuzzy_match_greedy(
        &mut self,
        haystack: Utf32Str<'_>,
        needle: Utf32Str<'_>,
    ) -> Option<u16> {
        assert!(haystack.len() <= u32::MAX as usize);
        self.fuzzy_match_greedy_impl::<false>(haystack, needle, &mut Vec::new())
    }

    pub fn fuzzy_indices_greedy(
        &mut self,
        haystack: Utf32Str<'_>,
        needle: Utf32Str<'_>,
        indidies: &mut Vec<u32>,
    ) -> Option<u16> {
        assert!(haystack.len() <= u32::MAX as usize);
        self.fuzzy_match_greedy_impl::<true>(haystack, needle, indidies)
    }

    fn fuzzy_match_greedy_impl<const INDICES: bool>(
        &mut self,
        haystack: Utf32Str<'_>,
        needle_: Utf32Str<'_>,
        indidies: &mut Vec<u32>,
    ) -> Option<u16> {
        if needle_.len() > haystack.len() || needle_.is_empty() {
            return None;
        }
        // if needle_.len() == haystack.len() {
        //     return self.exact_match();
        // }
        assert!(
            haystack.len() <= u32::MAX as usize,
            "fuzzy matching is only support for up to 2^32-1 codepoints"
        );
        match (haystack, needle_) {
            (Utf32Str::Ascii(haystack), Utf32Str::Ascii(needle)) => {
                let (start, greedy_end, _) = self.prefilter_ascii(haystack, needle, true)?;
                self.fuzzy_match_greedy_::<INDICES, AsciiChar, AsciiChar>(
                    AsciiChar::cast(haystack),
                    AsciiChar::cast(needle),
                    start,
                    greedy_end,
                    indidies,
                )
            }
            (Utf32Str::Ascii(_), Utf32Str::Unicode(_)) => {
                // a purely ascii haystack can never be transformed to match
                // a needle that contains non-ascii chars since we don't allow gaps
                None
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Ascii(needle)) => {
                let (start, _) = self.prefilter_non_ascii(haystack, needle_, true)?;
                self.fuzzy_match_greedy_::<INDICES, char, AsciiChar>(
                    haystack,
                    AsciiChar::cast(needle),
                    start,
                    start + 1,
                    indidies,
                )
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Unicode(needle)) => {
                let (start, _) = self.prefilter_non_ascii(haystack, needle_, true)?;
                self.fuzzy_match_greedy_::<INDICES, char, char>(
                    haystack,
                    needle,
                    start,
                    start + 1,
                    indidies,
                )
            }
        }
    }
}
