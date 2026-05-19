//! Prompt templates. Each prompt has a stable version suffix so model output
//! can be traced back to a specific revision via `questions.generated_by` /
//! `reports.prompt_version`.

pub const TECH_QUESTIONS_PROMPT_VERSION: &str = "tech_questions.v1";
pub const RUBRIC_PROMPT_VERSION: &str = "rubric.v1";
pub const SCORE_PROMPT_VERSION: &str = "score.v1";
pub const FOLLOWUP_PROMPT_VERSION: &str = "followup.v1";
pub const GRADE_PROMPT_VERSION: &str = "grade.v1";
pub const SUMMARY_PROMPT_VERSION: &str = "summary.v1";

pub fn tech_questions_system(tech_count: u16) -> String {
    format!(
        "You are an expert technical interviewer. Generate EXACTLY {tech_count} technical \
interview questions tailored to the candidate.\n\n\
PRIORITY: The Job Description is your PRIMARY source. Focus on the technologies, skills, \
responsibilities, and expectations LISTED IN THE JD. Cover the JD's tech stack thoroughly.\n\n\
The resume is SECONDARY: use it only to calibrate difficulty and occasionally connect a JD \
requirement to the candidate's past work. Do NOT ask deep questions about technologies on \
the resume that are not in the JD.\n\n\
Mix conceptual, scenario-based, and practical questions. Make some easy, some medium, some \
hard. Keep each question CONCISE — 1 to 2 sentences max, under 200 characters each.\n\n\
Return ONLY a JSON array of question strings — no preamble, no markdown, no code fences."
    )
}

pub fn tech_questions_user(jd_text: &str, resume_text: &str, tech_count: u16) -> String {
    format!(
        "<JOB_DESCRIPTION>\n{jd}\n</JOB_DESCRIPTION>\n\n\
<CANDIDATE_RESUME>\n{resume}\n</CANDIDATE_RESUME>\n\n\
Generate {tech_count} concise technical interview questions, prioritizing JD content.",
        jd = jd_text,
        resume = resume_text
    )
}

pub fn rubric_system() -> String {
    "You write concise scoring rubrics for mock interviewers. Output PLAIN TEXT only — \
no JSON, no markdown headers, no bullet stars. Maximum 150 words. Be specific to the role and \
the candidate's background. The rubric will be shown to a downstream scorer."
        .to_string()
}

pub fn rubric_user(jd_text: &str, resume_text: &str, role: &str) -> String {
    format!(
        "ROLE: {role}\n\n<JOB_DESCRIPTION>\n{jd}\n</JOB_DESCRIPTION>\n\n\
<CANDIDATE_RESUME>\n{resume}\n</CANDIDATE_RESUME>\n\n\
Write a scoring rubric for evaluating this candidate's interview answers.",
        role = role,
        jd = jd_text,
        resume = resume_text
    )
}

pub fn score_system() -> String {
    "You are an expert technical interviewer scoring a mock interview. You will score the \
candidate fairly and rigorously.\n\n\
Return ONLY a valid JSON object with this exact structure (no other text, no markdown):\n\
{\n  \"overall_percentage\": <0-100 number>,\n  \"technical_score\": <0-100>,\n  \
\"behavioral_score\": <0-100>,\n  \"strengths\": [\"...\", \"...\", \"...\"],\n  \
\"weaknesses\": [\"...\", \"...\", \"...\"],\n  \"action_items\": [\"...\", \"...\", \"...\"],\n  \
\"per_question\": [\n    {\"q\": <ordinal>, \"section\": \"Technical\"|\"Behavioral\"|\"Intro\", \
\"score\": <0-100>, \"feedback\": \"short feedback\"},\n    ...\n  ],\n  \
\"summary\": \"2-3 sentence overall summary\"\n}\n\n\
Scoring guidance:\n\
- Be rigorous. Score correctness, depth, clarity, and relevance.\n\
- 90% means truly excellent — strong technical depth, clear communication, well-structured answers.\n\
- Penalize vague, off-topic, or shallow answers.\n\
- For technical answers, weigh accuracy heavily.\n\
- For behavioral, look for STAR-style structure, specific examples, and self-awareness.\n\
- Overall percentage should reflect both sections weighted ~60% technical, ~40% behavioral. \
Intro answers may inform behavioral scoring but are not scored separately."
        .to_string()
}

pub fn score_user(
    role: &str,
    candidate_name: &str,
    rubric: &str,
    jd_text: &str,
    resume_text: &str,
    transcript: &str,
) -> String {
    let context = if !rubric.trim().is_empty() {
        format!("EVALUATION RUBRIC:\n{rubric}")
    } else {
        format!(
            "<JOB_DESCRIPTION>\n{jd_text}\n</JOB_DESCRIPTION>\n\n<CANDIDATE_RESUME>\n{resume_text}\n</CANDIDATE_RESUME>"
        )
    };
    format!(
        "{context}\n\nROLE: {role}\nCANDIDATE: {candidate_name}\n\nINTERVIEW TRANSCRIPT:\n{transcript}\n\n\
Score this interview and return the JSON evaluation."
    )
}

// ─── Phase 4: follow-ups + per-question grading ─────────────────────────────

pub fn followup_system() -> String {
    "You are an expert technical interviewer. Generate ONE follow-up question that probes the \
specific claim, design choice, or weakness in the candidate's answer.\n\n\
RULES:\n\
- Be specific to the answer — do not ask a generic probe.\n\
- Answerable in under 90 seconds of speech.\n\
- Do NOT repeat the original question.\n\
- If the answer was empty or non-substantive, ask the candidate to clarify their hands-on \
experience with the underlying topic.\n\
- One sentence, under 200 characters.\n\n\
Return ONLY this JSON, no prose, no markdown, no code fences:\n\
{\"question\": \"<one sentence>\", \"probe_target\": \"<3-6 word label of what you're testing>\"}"
        .to_string()
}

pub fn followup_user(
    role: &str,
    jd_text: &str,
    primary_question: &str,
    answer_transcript: &str,
) -> String {
    let jd_excerpt = if jd_text.len() > 1200 {
        &jd_text[..1200]
    } else {
        jd_text
    };
    let answer = if answer_transcript.trim().is_empty() {
        "(no answer recorded)"
    } else {
        answer_transcript
    };
    format!(
        "ROLE: {role}\n\n<JOB_DESCRIPTION_EXCERPT>\n{jd_excerpt}\n</JOB_DESCRIPTION_EXCERPT>\n\n\
The candidate was just asked:\nQ: {primary_question}\n\nThey answered:\nA: {answer}\n\n\
Generate one tightly-targeted follow-up question."
    )
}

pub fn grade_system() -> String {
    "You are an expert interviewer grading a single answer in a mock interview. Return a \
numeric score and a short reasoning.\n\n\
RULES:\n\
- Score 0-100. 90+ means excellent: accurate, deep, well-structured.\n\
- Penalize vague, off-topic, or shallow answers.\n\
- For Technical/Follow-up answers, weigh accuracy and depth heavily.\n\
- For Behavioral answers, look for STAR-style structure and specific examples.\n\
- For Intro answers, score on clarity and relevance; do not over-weight.\n\
- If the answer is empty or evasive, score low and say so.\n\n\
Return ONLY this JSON, no prose, no markdown:\n\
{\"score\": <0-100 integer>, \"reasoning\": \"<one or two sentences>\"}"
        .to_string()
}

pub fn grade_user(
    section: &str,
    question_text: &str,
    answer_transcript: &str,
    rubric: &str,
    role: &str,
) -> String {
    let context = if rubric.trim().is_empty() {
        String::new()
    } else {
        format!("EVALUATION RUBRIC:\n{rubric}\n\n")
    };
    let answer = if answer_transcript.trim().is_empty() {
        "(no answer recorded)"
    } else {
        answer_transcript
    };
    format!(
        "{context}ROLE: {role}\nSECTION: {section}\n\nQ: {question_text}\nA: {answer}\n\n\
Grade this single answer and return the JSON."
    )
}

pub fn summary_system() -> String {
    "You are an expert interviewer writing the narrative section of a final mock-interview \
report. You will be given pre-computed per-question scores and reasonings. Your job is to \
synthesize them — do NOT re-score, do NOT contradict the existing scores.\n\n\
Return ONLY this JSON, no markdown, no code fences, no prose outside the object:\n\
{\n  \"strengths\": [\"...\", \"...\", \"...\"],\n  \"weaknesses\": [\"...\", \"...\", \"...\"],\n  \
\"action_items\": [\"...\", \"...\", \"...\"],\n  \"summary\": \"2-3 sentence overall summary\"\n}\n\n\
Guidance:\n\
- 3 to 5 items in each list.\n\
- Strengths/weaknesses should reference concrete topics or answers, not generic phrases.\n\
- Action items should be specific, achievable next steps for the candidate.\n\
- Summary should reference the role and the candidate's overall trajectory."
        .to_string()
}

pub fn summary_user(
    role: &str,
    candidate_name: &str,
    rubric: &str,
    technical_score: i16,
    behavioral_score: i16,
    overall: i16,
    graded_questions_json: &str,
) -> String {
    let context = if rubric.trim().is_empty() {
        String::new()
    } else {
        format!("EVALUATION RUBRIC:\n{rubric}\n\n")
    };
    format!(
        "{context}ROLE: {role}\nCANDIDATE: {candidate_name}\n\n\
PRE-COMPUTED SCORES (do not re-score):\n\
- Technical: {technical_score}\n- Behavioral: {behavioral_score}\n- Overall: {overall}\n\n\
PER-QUESTION GRADES (JSON):\n{graded_questions_json}\n\n\
Synthesize the final report's narrative sections and return the JSON object."
    )
}
