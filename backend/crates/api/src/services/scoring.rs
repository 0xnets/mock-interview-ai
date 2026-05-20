use persistence::repo_realtime::GradedQuestion;

use crate::models::interviews::PerQuestionEntry;

/// Returns (technical, behavioral, overall) on a 0-100 scale. Technical
/// includes `followup` questions. Intro is excluded from both sub-scores
/// because the intro prompt is fixed and not a skill signal — but if neither
/// section yielded a graded question, we degrade gracefully by averaging
/// whatever grades we have.
pub fn compute_scores(graded: &[GradedQuestion]) -> (i16, i16, i16) {
    let mut tech_sum = 0i32;
    let mut tech_n = 0i32;
    let mut beh_sum = 0i32;
    let mut beh_n = 0i32;
    let mut other_sum = 0i32;
    let mut other_n = 0i32;
    for q in graded {
        let Some(g) = &q.grade else { continue };
        match q.kind.as_str() {
            "technical" | "followup" => {
                tech_sum += g.score as i32;
                tech_n += 1;
            }
            "behavioral" => {
                beh_sum += g.score as i32;
                beh_n += 1;
            }
            _ => {
                other_sum += g.score as i32;
                other_n += 1;
            }
        }
    }

    let technical = if tech_n > 0 {
        ((tech_sum + tech_n / 2) / tech_n) as i16
    } else if beh_n > 0 {
        ((beh_sum + beh_n / 2) / beh_n) as i16
    } else if other_n > 0 {
        ((other_sum + other_n / 2) / other_n) as i16
    } else {
        0
    };
    let behavioral = if beh_n > 0 {
        ((beh_sum + beh_n / 2) / beh_n) as i16
    } else if tech_n > 0 {
        ((tech_sum + tech_n / 2) / tech_n) as i16
    } else if other_n > 0 {
        ((other_sum + other_n / 2) / other_n) as i16
    } else {
        0
    };

    let overall = match (tech_n, beh_n) {
        (0, 0) => {
            if other_n > 0 {
                ((other_sum + other_n / 2) / other_n) as i16
            } else {
                0
            }
        }
        (_, 0) => technical,
        (0, _) => behavioral,
        _ => {
            let blended = (technical as f64) * 0.6 + (behavioral as f64) * 0.4;
            blended.round().clamp(0.0, 100.0) as i16
        }
    };

    (technical, behavioral, overall)
}

pub fn build_per_question(graded: &[GradedQuestion]) -> Vec<PerQuestionEntry> {
    graded
        .iter()
        .map(|q| {
            let section = match q.kind.as_str() {
                "intro" => "Intro",
                "technical" => "Technical",
                "behavioral" => "Behavioral",
                "followup" => "Follow-up",
                _ => "Question",
            };
            let duration_ms = q.duration_ms;
            let duration_seconds = duration_ms.map(|d| (d as i64) / 1000);
            let time_bucket = duration_ms.map(time_bucket);
            PerQuestionEntry {
                q: q.ordinal,
                section,
                question: q.prompt_text.clone(),
                score: q.grade.as_ref().map(|g| g.score),
                feedback: q
                    .grade
                    .as_ref()
                    .map(|g| g.reasoning.clone())
                    .unwrap_or_default(),
                duration_ms,
                duration_seconds,
                time_bucket,
            }
        })
        .collect()
}

/// Heuristic bands for how long a candidate took to answer a single question.
/// Used as report metadata only — never feeds back into grading.
pub fn time_bucket(duration_ms: i32) -> &'static str {
    match duration_ms {
        i32::MIN..=14_999 => "very_short",
        15_000..=120_000 => "normal",
        120_001..=240_000 => "long",
        _ => "very_long",
    }
}
