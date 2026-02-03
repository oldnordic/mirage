//! Source location mapping for CFG blocks

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Source location for a CFG block or statement
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    /// File path (relative to crate root or absolute)
    pub file_path: PathBuf,
    /// Byte offset of the start
    pub byte_start: usize,
    /// Byte offset of the end
    pub byte_end: usize,
    /// Line number (1-indexed)
    pub start_line: usize,
    /// Column number (1-indexed, in UTF-8 characters)
    pub start_column: usize,
    /// End line number
    pub end_line: usize,
    /// End column number
    pub end_column: usize,
}

impl SourceLocation {
    /// Create a new source location
    pub fn new(
        file_path: impl Into<PathBuf>,
        byte_start: usize,
        byte_end: usize,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            byte_start,
            byte_end,
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }

    /// Convert byte offsets to line/column
    ///
    /// This is a simplified implementation. For production use,
    /// you'd want to cache line endings and handle edge cases.
    pub fn from_bytes(file_path: impl Into<PathBuf>, source: &str, byte_start: usize, byte_end: usize) -> Self {
        let (start_line, start_col) = byte_to_line_column(source, byte_start);
        let (end_line, end_col) = byte_to_line_column(source, byte_end);

        Self {
            file_path: file_path.into(),
            byte_start,
            byte_end,
            start_line,
            start_column: start_col,
            end_line,
            end_column: end_col,
        }
    }

    /// Get a human-readable description
    pub fn display(&self) -> String {
        format!(
            "{}:{}:{}-{}:{}",
            self.file_path.display(),
            self.start_line,
            self.start_column,
            self.end_line,
            self.end_column
        )
    }

    /// Check if this location overlaps with another
    pub fn overlaps(&self, other: &SourceLocation) -> bool {
        if self.file_path != other.file_path {
            return false;
        }
        // Overlap if ranges intersect
        self.byte_start < other.byte_end && self.byte_end > other.byte_start
    }

    /// Create a source location from byte ranges, with optional source for line/column.
    ///
    /// If source is provided, computes line/column from byte offsets.
    /// If source is None, line/column fields are set to 0 (lazy computation).
    ///
    /// This is useful when reconstructing SourceLocation from database where
    /// the source file may not be available.
    pub fn from_bytes_with_source(
        file_path: impl Into<PathBuf>,
        source: Option<&str>,
        byte_start: usize,
        byte_end: usize,
    ) -> Self {
        let file_path = file_path.into();

        if let Some(src) = source {
            // Compute line/column from source
            let (start_line, start_col) = byte_to_line_column(src, byte_start);
            let (end_line, end_col) = byte_to_line_column(src, byte_end);
            Self {
                file_path,
                byte_start,
                byte_end,
                start_line,
                start_column: start_col,
                end_line,
                end_column: end_col,
            }
        } else {
            // No source available - line/column will be 0
            // Display will fall back to byte ranges
            Self {
                file_path,
                byte_start,
                byte_end,
                start_line: 0,
                start_column: 0,
                end_line: 0,
                end_column: 0,
            }
        }
    }

    /// Get a human-readable description (fallback to byte ranges if line/column unavailable)
    pub fn display_or_bytes(&self) -> String {
        if self.start_line > 0 {
            self.display()
        } else {
            format!(
                "{}:bytes{}-{}",
                self.file_path.display(),
                self.byte_start,
                self.byte_end
            )
        }
    }
}

/// Convert byte offset to line and column (1-indexed)
fn byte_to_line_column(source: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    let mut current_byte = 0;

    for ch in source.chars() {
        if current_byte >= byte_offset {
            break;
        }

        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }

        current_byte += ch.len_utf8();
    }

    (line, column)
}

/// Span from Charon ULLBC
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CharonSpan {
    pub file_id: usize,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl SourceLocation {
    /// Convert from Charon span
    ///
    /// Note: Charon uses file IDs, not paths. You'll need to
    /// map file IDs to paths using the ULLBC file table.
    pub fn from_charon_span(
        file_path: PathBuf,
        span: &CharonSpan,
    ) -> Self {
        Self {
            file_path,
            byte_start: 0,  // Charon doesn't provide bytes
            byte_end: 0,
            start_line: span.start_line,
            start_column: span.start_column,
            end_line: span.end_line,
            end_column: span.end_column,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_to_line_column() {
        let source = "line 1\nline 2\nline 3";

        assert_eq!(byte_to_line_column(source, 0), (1, 1));
        assert_eq!(byte_to_line_column(source, 6), (1, 7));
        assert_eq!(byte_to_line_column(source, 7), (2, 1));
        assert_eq!(byte_to_line_column(source, 13), (2, 7));
        assert_eq!(byte_to_line_column(source, 14), (3, 1));
    }

    #[test]
    fn test_source_location_from_bytes() {
        let source = "hello\nworld";
        let loc = SourceLocation::from_bytes("test.rs", source, 0, 5);

        assert_eq!(loc.start_line, 1);
        assert_eq!(loc.start_column, 1);
        assert_eq!(loc.end_line, 1);
        assert_eq!(loc.end_column, 6);
    }

    #[test]
    fn test_source_location_display() {
        let loc = SourceLocation {
            file_path: PathBuf::from("src/test.rs"),
            byte_start: 0,
            byte_end: 10,
            start_line: 5,
            start_column: 3,
            end_line: 5,
            end_column: 13,
        };

        assert_eq!(loc.display(), "src/test.rs:5:3-5:13");
    }

    #[test]
    fn test_overlaps() {
        let loc1 = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 0,
            byte_end: 10,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 11,
        };

        let loc2 = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 5,
            byte_end: 15,
            start_line: 1,
            start_column: 6,
            end_line: 1,
            end_column: 16,
        };

        assert!(loc1.overlaps(&loc2));

        let loc3 = SourceLocation {
            file_path: PathBuf::from("other.rs"),
            byte_start: 0,
            byte_end: 10,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 11,
        };

        assert!(!loc1.overlaps(&loc3)); // Different file
    }

    #[test]
    fn test_source_location_new() {
        let loc = SourceLocation::new(
            "path/to/file.rs",
            100,
            200,
            10,
            5,
            15,
            20,
        );

        assert_eq!(loc.file_path, PathBuf::from("path/to/file.rs"));
        assert_eq!(loc.byte_start, 100);
        assert_eq!(loc.byte_end, 200);
        assert_eq!(loc.start_line, 10);
        assert_eq!(loc.start_column, 5);
        assert_eq!(loc.end_line, 15);
        assert_eq!(loc.end_column, 20);
    }

    #[test]
    fn test_from_charon_span() {
        let span = CharonSpan {
            file_id: 0,
            start_line: 5,
            start_column: 3,
            end_line: 7,
            end_column: 10,
        };

        let loc = SourceLocation::from_charon_span(PathBuf::from("src/lib.rs"), &span);

        assert_eq!(loc.file_path, PathBuf::from("src/lib.rs"));
        assert_eq!(loc.byte_start, 0); // Charon doesn't provide bytes
        assert_eq!(loc.byte_end, 0);
        assert_eq!(loc.start_line, 5);
        assert_eq!(loc.start_column, 3);
        assert_eq!(loc.end_line, 7);
        assert_eq!(loc.end_column, 10);
    }

    #[test]
    fn test_multibyte_character_handling() {
        // Test UTF-8 multibyte character handling
        // "hello 世界\nworld" has:
        // "hello " = 6 bytes (h=1, e=1, l=1, l=1, o=1, space=1)
        // "世" = 3 bytes (UTF-8), starts at byte 6
        // "界" = 3 bytes (UTF-8), starts at byte 9
        // "\n" = 1 byte, starts at byte 12
        // Total to end of line 1 = 13 bytes (0-12)

        // At byte 0: column 1 (h)
        // At byte 6: column 7 (first byte of "世")
        // At byte 12: column 8 (newline)
        // At byte 13: line 2, column 1 (w)

        let source = "hello 世界\nworld";

        let (line, col) = byte_to_line_column(source, 0);
        assert_eq!(line, 1);
        assert_eq!(col, 1);

        let (line, col) = byte_to_line_column(source, 6);
        assert_eq!(line, 1);
        assert_eq!(col, 7); // h e l l o space = 6 chars processed, next is 7th

        let (line, col) = byte_to_line_column(source, 13);
        assert_eq!(line, 2);
        assert_eq!(col, 1); // w on line 2
    }

    #[test]
    fn test_overlaps_adjacent_no_overlap() {
        // Adjacent ranges do not overlap
        let loc1 = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 0,
            byte_end: 10,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 11,
        };

        let loc2 = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 10,
            byte_end: 20,
            start_line: 1,
            start_column: 11,
            end_line: 1,
            end_column: 21,
        };

        assert!(!loc1.overlaps(&loc2));
    }

    #[test]
    fn test_overlaps_contained() {
        // One range contained within another
        let loc1 = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 0,
            byte_end: 100,
            start_line: 1,
            start_column: 1,
            end_line: 5,
            end_column: 1,
        };

        let loc2 = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 20,
            byte_end: 30,
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 11,
        };

        assert!(loc1.overlaps(&loc2));
        assert!(loc2.overlaps(&loc1));
    }

    #[test]
    fn test_from_bytes_with_source_with_source() {
        let source = "hello\nworld";
        let loc = SourceLocation::from_bytes_with_source("test.rs", Some(source), 0, 5);

        assert_eq!(loc.file_path, PathBuf::from("test.rs"));
        assert_eq!(loc.byte_start, 0);
        assert_eq!(loc.byte_end, 5);
        assert_eq!(loc.start_line, 1);
        assert_eq!(loc.start_column, 1);
        assert_eq!(loc.end_line, 1);
        assert_eq!(loc.end_column, 6);
    }

    #[test]
    fn test_from_bytes_with_source_without_source() {
        let loc = SourceLocation::from_bytes_with_source("test.rs", None, 10, 20);

        assert_eq!(loc.file_path, PathBuf::from("test.rs"));
        assert_eq!(loc.byte_start, 10);
        assert_eq!(loc.byte_end, 20);
        assert_eq!(loc.start_line, 0);  // No source = 0
        assert_eq!(loc.start_column, 0);
        assert_eq!(loc.end_line, 0);
        assert_eq!(loc.end_column, 0);
    }

    #[test]
    fn test_display_or_bytes_with_line_column() {
        let loc = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 0,
            byte_end: 10,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 11,
        };

        assert_eq!(loc.display_or_bytes(), "test.rs:1:1-1:11");
    }

    #[test]
    fn test_display_or_bytes_without_line_column() {
        let loc = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 100,
            byte_end: 200,
            start_line: 0,
            start_column: 0,
            end_line: 0,
            end_column: 0,
        };

        assert_eq!(loc.display_or_bytes(), "test.rs:bytes100-200");
    }
}
