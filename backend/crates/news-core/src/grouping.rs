use std::collections::HashSet;

/// Extract character trigrams from a string (works well with Japanese + English).
pub fn trigrams(s: &str) -> HashSet<String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 3 {
        let mut set = HashSet::new();
        if !chars.is_empty() {
            set.insert(chars.iter().collect());
        }
        return set;
    }
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

/// Jaccard similarity between two strings based on character trigrams.
pub fn similarity(a: &str, b: &str) -> f64 {
    let ta = trigrams(a);
    let tb = trigrams(b);
    if ta.is_empty() && tb.is_empty() {
        return 1.0;
    }
    let intersection = ta.intersection(&tb).count();
    let union = ta.union(&tb).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

/// Group articles by title similarity using union-find.
/// Returns groups as Vec<Vec<usize>> where each inner vec contains article indices.
/// Articles with similarity >= threshold are grouped together.
pub fn group_articles(titles: &[&str], threshold: f64) -> Vec<Vec<usize>> {
    let n = titles.len();
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }

    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[rb] = ra;
        }
    }

    // Compare all pairs
    for i in 0..n {
        for j in (i + 1)..n {
            if similarity(titles[i], titles[j]) >= threshold {
                union(&mut parent, i, j);
            }
        }
    }

    // Collect groups
    let mut groups: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(i);
    }

    groups.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigrams_basic() {
        let t = trigrams("abcde");
        assert!(t.contains("abc"));
        assert!(t.contains("bcd"));
        assert!(t.contains("cde"));
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn trigrams_japanese() {
        let t = trigrams("東京都知事");
        assert!(t.contains("東京都"));
        assert!(t.contains("京都知"));
        assert!(t.contains("都知事"));
    }

    #[test]
    fn trigrams_short() {
        let t = trigrams("ab");
        assert_eq!(t.len(), 1);
        assert!(t.contains("ab"));
    }

    #[test]
    fn similarity_identical() {
        assert!((similarity("hello world", "hello world") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn similarity_different() {
        let s = similarity("hello", "xyz123abc");
        assert!(s < 0.1);
    }

    #[test]
    fn similarity_similar_titles() {
        let s = similarity(
            "東京都で新型コロナウイルスの感染者が100人確認",
            "東京都で新型コロナウイルスの感染者が150人確認",
        );
        assert!(s > 0.5, "Similar Japanese titles should have high similarity: {}", s);
    }

    #[test]
    fn group_articles_basic() {
        let titles = vec![
            "東京都で新型コロナ100人確認",
            "東京都で新型コロナ150人確認",
            "サッカーW杯の結果速報",
            "プログラミング言語Rustの最新版",
        ];
        let groups = group_articles(
            &titles.iter().map(|s| *s).collect::<Vec<_>>(),
            0.3,
        );
        // The two corona articles should be grouped together
        let has_corona_group = groups.iter().any(|g| g.contains(&0) && g.contains(&1));
        assert!(has_corona_group, "Similar articles should be grouped: {:?}", groups);
    }

    #[test]
    fn group_articles_no_groups() {
        let titles = vec!["aaa", "bbb", "ccc"];
        let groups = group_articles(&titles.iter().map(|s| *s).collect::<Vec<_>>(), 0.5);
        // All should be separate
        assert_eq!(groups.len(), 3);
    }
}
