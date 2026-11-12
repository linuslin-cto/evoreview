//! 跨任务 improvement library（Harnessing-level 记忆）。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lesson {
    pub id: String,
    pub when: String,
    pub change: String,
    pub why: String,
    pub evidence: String,
    #[serde(default)]
    pub adopted_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImprovementLibrary {
    pub lessons: Vec<Lesson>,
    #[serde(default)]
    pub adopted_harnesses: Vec<String>,
}

impl ImprovementLibrary {
    pub fn path_from_env() -> PathBuf {
        std::env::var("HARNESS2_LIBRARY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data/harness2_library.json"))
    }

    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn load_from_env() -> Self {
        Self::load(&Self::path_from_env())
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let s = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, s).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save_from_env(&self) -> Result<(), String> {
        self.save(&Self::path_from_env())
    }

    pub fn top_lessons(&self, n: usize) -> Vec<&Lesson> {
        let mut items: Vec<_> = self.lessons.iter().collect();
        items.sort_by_key(|a| std::cmp::Reverse(a.adopted_count));
        items.into_iter().take(n).collect()
    }

    pub fn render_hints(&self, n: usize) -> String {
        let tops = self.top_lessons(n);
        if tops.is_empty() {
            return String::new();
        }
        let mut out = String::from("### Harness² 课程（历史有效编辑）\n");
        for (i, l) in tops.iter().enumerate() {
            out.push_str(&format!(
                "{}. 当 {} → {}（因 {}；证据：{}）\n",
                i + 1,
                l.when,
                l.change,
                l.why,
                l.evidence
            ));
        }
        out
    }

    pub fn upsert_lesson(&mut self, lesson: Lesson) {
        if let Some(existing) = self.lessons.iter_mut().find(|l| l.id == lesson.id) {
            existing.adopted_count = existing.adopted_count.saturating_add(1);
            existing.evidence = lesson.evidence;
            existing.why = lesson.why;
            existing.change = lesson.change;
            existing.when = lesson.when;
        } else {
            self.lessons.push(lesson);
        }
    }

    pub fn record_adoption(&mut self, harness_summary: String, lesson: Lesson) {
        self.adopted_harnesses.push(harness_summary);
        if self.adopted_harnesses.len() > 50 {
            let drain = self.adopted_harnesses.len() - 50;
            self.adopted_harnesses.drain(0..drain);
        }
        self.upsert_lesson(lesson);
        info!(lessons = self.lessons.len(), "harness2: library updated");
        if let Err(e) = self.save_from_env() {
            warn!(error = %e, "harness2: library save failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn roundtrip_file() {
        let dir = std::env::temp_dir().join(format!(
            "h2lib-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lib.json");
        let mut lib = ImprovementLibrary::default();
        lib.upsert_lesson(Lesson {
            id: "anchor".into(),
            when: "定位差".into(),
            change: "强制引用 diff 行".into(),
            why: "提升锚定率".into(),
            evidence: "anchored_rate 0.2→0.8".into(),
            adopted_count: 2,
        });
        lib.save(&path).unwrap();
        let loaded = ImprovementLibrary::load(&path);
        assert_eq!(loaded.lessons.len(), 1);
        assert!(loaded.render_hints(3).contains("定位差"));
        let _ = fs::remove_dir_all(&dir);
    }
}
