//! Copy election across streams (architecture §8.1 step 3).
//!
//! A record counts as copied if its provider-native identity was also seen in
//! another stream. Without a validated producer copy marker, the canonical
//! occurrence is chosen in this order:
//!
//! 1. **containment**: a stream whose identified records all appear in the
//!    other stream is the original (the usual resume case);
//! 2. **stream creation**: the stream whose file was created first, where
//!    both creation times are known, else the lowest observation `seq`;
//! 3. the stream key.
//!
//! Copies are never deleted. The caller marks them `copied_from` /
//! `duplicate_of` and derives nothing from them.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::time::UtcMillis;

/// What election needs to know about one stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamOrder {
    pub stream_key: String,
    /// Creation time of the stream's file where the OS provides it.
    pub birth: Option<UtcMillis>,
    /// Lowest observation `seq` of the stream.
    pub min_seq: i64,
}

/// Canonical-stream decisions for identities seen in more than one stream.
#[derive(Debug, Clone)]
pub struct CopyElection {
    streams: Vec<StreamOrder>,
    /// Identified record count per stream.
    record_counts: Vec<usize>,
    /// Shared identified record count per stream pair (a < b).
    shared: BTreeMap<(usize, usize), usize>,
    /// Record identity → canonical stream, only for identities in > 1 stream.
    records: BTreeMap<String, usize>,
}

impl CopyElection {
    /// `records[i]` are the identified native record IDs of stream `i`.
    pub fn new(streams: Vec<StreamOrder>, records: &[BTreeSet<String>]) -> Self {
        assert_eq!(streams.len(), records.len());
        let mut holders: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (i, ids) in records.iter().enumerate() {
            for id in ids {
                holders.entry(id.as_str()).or_default().push(i);
            }
        }
        let mut shared: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        for group in holders.values().filter(|g| g.len() > 1) {
            for (n, &a) in group.iter().enumerate() {
                for &b in &group[n + 1..] {
                    *shared.entry((a.min(b), a.max(b))).or_default() += 1;
                }
            }
        }
        let mut election = Self {
            streams,
            record_counts: records.iter().map(BTreeSet::len).collect(),
            shared,
            records: BTreeMap::new(),
        };
        let decided: Vec<(String, usize)> = holders
            .iter()
            .filter(|(_, g)| g.len() > 1)
            .map(|(id, g)| ((*id).to_owned(), election.canonical(g)))
            .collect();
        election.records.extend(decided);
        election
    }

    fn contained(&self, x: usize, group: &[usize]) -> bool {
        self.record_counts[x] > 0
            && group.iter().filter(|&&y| y != x).all(|&y| {
                self.shared.get(&(x.min(y), x.max(y))).copied().unwrap_or(0)
                    == self.record_counts[x]
            })
    }

    fn compare(&self, a: usize, b: usize, group: &[usize]) -> Ordering {
        let (ca, cb) = (self.contained(a, group), self.contained(b, group));
        if ca != cb {
            return if ca {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let (sa, sb) = (&self.streams[a], &self.streams[b]);
        let by_creation = match (sa.birth, sb.birth) {
            (Some(x), Some(y)) if x != y => x.cmp(&y),
            _ => sa.min_seq.cmp(&sb.min_seq),
        };
        by_creation.then_with(|| sa.stream_key.cmp(&sb.stream_key))
    }

    /// The canonical stream among `group` (streams that share an identity).
    pub fn canonical(&self, group: &[usize]) -> usize {
        let mut sorted: Vec<usize> = group.to_vec();
        sorted.sort_by(|&a, &b| self.streams[a].stream_key.cmp(&self.streams[b].stream_key));
        sorted
            .into_iter()
            .reduce(|best, s| {
                if self.compare(s, best, group) == Ordering::Less {
                    s
                } else {
                    best
                }
            })
            .expect("non-empty group")
    }

    /// The canonical stream of a record identity seen in more than one
    /// stream, or `None` if only one stream holds it.
    pub fn canonical_record_stream(&self, record_id: &str) -> Option<usize> {
        self.records.get(record_id).copied()
    }

    /// Number of record identities seen in more than one stream.
    pub fn shared_records(&self) -> usize {
        self.records.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(key: &str, birth: Option<i64>, min_seq: i64) -> StreamOrder {
        StreamOrder {
            stream_key: key.into(),
            birth: birth.map(UtcMillis),
            min_seq,
        }
    }

    fn ids(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn resume_original_never_continued_is_contained_and_canonical() {
        // The copy was created *earlier* than the original here, so only
        // containment can pick the right stream.
        let e = CopyElection::new(
            vec![s("orig", Some(200), 5), s("copy", Some(100), 1)],
            &[ids(&["r1", "r2"]), ids(&["r1", "r2", "r3"])],
        );
        assert_eq!(e.canonical_record_stream("r1"), Some(0));
        assert_eq!(e.canonical_record_stream("r3"), None);
    }

    #[test]
    fn fork_both_continued_falls_back_to_creation_time() {
        let e = CopyElection::new(
            vec![s("b", Some(300), 1), s("a", Some(100), 9)],
            &[ids(&["r1", "x"]), ids(&["r1", "y"])],
        );
        assert_eq!(e.canonical_record_stream("r1"), Some(1));
    }

    #[test]
    fn missing_birth_uses_seq_then_key() {
        let e = CopyElection::new(
            vec![s("b", None, 7), s("a", Some(100), 9)],
            &[ids(&["r1", "x"]), ids(&["r1", "y"])],
        );
        assert_eq!(e.canonical_record_stream("r1"), Some(0));
        let tie = CopyElection::new(
            vec![s("b", None, 7), s("a", None, 7)],
            &[ids(&["r1", "x"]), ids(&["r1", "y"])],
        );
        assert_eq!(tie.canonical_record_stream("r1"), Some(1));
    }

    #[test]
    fn election_ignores_input_order() {
        let a = CopyElection::new(
            vec![s("x", Some(1), 1), s("y", Some(2), 2), s("z", Some(3), 3)],
            &[ids(&["r", "p"]), ids(&["r", "q"]), ids(&["r"])],
        );
        let b = CopyElection::new(
            vec![s("z", Some(3), 3), s("y", Some(2), 2), s("x", Some(1), 1)],
            &[ids(&["r"]), ids(&["r", "q"]), ids(&["r", "p"])],
        );
        // z is contained in both others: it is the original in either order.
        assert_eq!(a.canonical_record_stream("r"), Some(2));
        assert_eq!(b.canonical_record_stream("r"), Some(0));
    }
}
