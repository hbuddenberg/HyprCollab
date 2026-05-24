/// Compute a unified-diff string between `old` and `new`.
///
/// Returns an empty string when the two inputs are identical.
/// Uses the LCS algorithm for minimal edit distance.
pub fn unified_diff(old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }

    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let matches = lcs_indices(&old_lines, &new_lines);

    let mut result = String::new();
    result.push_str("--- a\n+++ b\n");
    result.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        1,
        old_lines.len(),
        1,
        new_lines.len()
    ));

    let mut old_i = 0usize;
    let mut new_i = 0usize;
    let mut match_iter = matches.iter().peekable();

    loop {
        match match_iter.peek() {
            Some(&&(oi, ni)) => {
                // deletions before this common line
                while old_i < oi {
                    result.push('-');
                    result.push_str(old_lines[old_i]);
                    result.push('\n');
                    old_i += 1;
                }
                // insertions before this common line
                while new_i < ni {
                    result.push('+');
                    result.push_str(new_lines[new_i]);
                    result.push('\n');
                    new_i += 1;
                }
                // context line
                result.push(' ');
                result.push_str(old_lines[old_i]);
                result.push('\n');
                old_i += 1;
                new_i += 1;
                match_iter.next();
            }
            None => break,
        }
    }

    // remaining deletions
    while old_i < old_lines.len() {
        result.push('-');
        result.push_str(old_lines[old_i]);
        result.push('\n');
        old_i += 1;
    }
    // remaining insertions
    while new_i < new_lines.len() {
        result.push('+');
        result.push_str(new_lines[new_i]);
        result.push('\n');
        new_i += 1;
    }

    result
}

/// Returns the (old_index, new_index) pairs of the longest common subsequence.
fn lcs_indices<T: Eq>(a: &[T], b: &[T]) -> Vec<(usize, usize)> {
    let m = a.len();
    let n = b.len();

    let mut dp = vec![0usize; (m + 1) * (n + 1)];

    for i in 1..=m {
        for j in 1..=n {
            dp[i * (n + 1) + j] = if a[i - 1] == b[j - 1] {
                dp[(i - 1) * (n + 1) + (j - 1)] + 1
            } else {
                dp[(i - 1) * (n + 1) + j].max(dp[i * (n + 1) + (j - 1)])
            };
        }
    }

    // backtrack
    let mut result = Vec::new();
    let (mut i, mut j) = (m, n);
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            result.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[(i - 1) * (n + 1) + j] >= dp[i * (n + 1) + (j - 1)] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    result.reverse();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_content_returns_empty() {
        assert_eq!(unified_diff("hello\nworld\n", "hello\nworld\n"), "");
    }

    #[test]
    fn both_empty_returns_empty() {
        assert_eq!(unified_diff("", ""), "");
    }

    #[test]
    fn added_line_marked_with_plus() {
        let d = unified_diff("line1\n", "line1\nline2\n");
        assert!(d.contains("+line2"), "diff:\n{d}");
    }

    #[test]
    fn removed_line_marked_with_minus() {
        let d = unified_diff("line1\nline2\n", "line1\n");
        assert!(d.contains("-line2"), "diff:\n{d}");
    }

    #[test]
    fn mixed_changes_contain_both_markers() {
        let old = "a\nb\nc\n";
        let new = "a\nX\nc\n";
        let d = unified_diff(old, new);
        assert!(d.contains("-b"), "diff:\n{d}");
        assert!(d.contains("+X"), "diff:\n{d}");
    }

    #[test]
    fn context_lines_marked_with_space() {
        let d = unified_diff("a\nb\n", "a\nX\n");
        assert!(d.contains(" a"), "unchanged line should be context: {d}");
    }

    #[test]
    fn diff_has_header() {
        let d = unified_diff("a\n", "b\n");
        assert!(d.starts_with("--- a\n"), "should have header: {d}");
        assert!(d.contains("+++ b\n"), "should have header: {d}");
        assert!(d.contains("@@"), "should have hunk header: {d}");
    }

    #[test]
    fn complete_replacement() {
        let d = unified_diff("foo\n", "bar\n");
        assert!(d.contains("-foo"), "diff:\n{d}");
        assert!(d.contains("+bar"), "diff:\n{d}");
    }
}
