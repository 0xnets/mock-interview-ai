import { state } from '../app/state.js';
import { envNumber } from '../app/env.js';
import { callAI } from '../services/gemini.service.js';
import { extractJSON, normalizeQuestionArray } from '../utils/json.js';
import { pickNonTechQuestions } from './non-tech-bank.js';

export async function prepareCandidateSession(name, role, jd, resume, statusCallback) {
  if (!state.config.apiKey) throw new Error('API key not configured. Set it in HR Configuration.');
  if (!state.config.nonTechBank) throw new Error('Non-tech question bank is empty. Set it in HR Configuration.');

  statusCallback && statusCallback('🧠 Generating personalized technical questions...');

  // 1. Generate tech questions tailored to this candidate
  const techSys = `You are an expert technical interviewer. Generate exactly ${state.config.techCount} technical interview questions.

PRIORITY: The Job Description is your PRIMARY source. Focus on the technologies, skills, responsibilities, and expectations LISTED IN THE JD. Cover the JD's tech stack thoroughly.

The resume should be used SECONDARILY — only to calibrate question difficulty (e.g., go deeper if the candidate has years of experience with a JD-required tech) and to occasionally connect a JD requirement to the candidate's past work. Do NOT ask deep questions about technologies on the resume that aren't in the JD.

Mix conceptual, scenario-based, and practical questions. Make some easy, some medium, some hard. Keep each question CONCISE — 1 to 2 sentences max, under 200 characters each.

Return ONLY a JSON array of question strings.`;
  const techUser = `JOB DESCRIPTION (primary focus):\n${jd}\n\nCANDIDATE RESUME (for context only):\n${resume}\n\nGenerate ${state.config.techCount} concise technical interview questions, prioritizing JD content.`;
  const techSchema = { type: 'ARRAY', items: { type: 'STRING' } };
  const techRes = await callAI(techSys, techUser, envNumber('GEMINI_DEFAULT_MAX_OUTPUT_TOKENS'), techSchema, false);
  const techQs = normalizeQuestionArray(extractJSON(techRes));
  if (techQs.length === 0) throw new Error('AI returned no questions. Check the JD/resume are not empty.');

  statusCallback && statusCallback('📋 Building scoring rubric...');

  // 2. Generate a brief scoring context (the only thing the scorer needs later)
  const ctxSys = `Write a concise scoring rubric (max 150 words) listing what an interviewer should look for when evaluating this candidate's answers. Be specific to the role and the candidate's background. Return plain text only — no JSON, no markdown.`;
  const ctxUser = `JOB DESCRIPTION:\n${jd}\n\nCANDIDATE RESUME:\n${resume}`;
  const ctxRes = await callAI(ctxSys, ctxUser, envNumber('GEMINI_RUBRIC_MAX_OUTPUT_TOKENS'), null, false);
  const scoringContext = ctxRes.replace(/```[a-z]*\n?/gi, '').replace(/```/g, '').trim();

  // 3. Pre-pick non-tech questions (1 per category if categorized, else random N)
  const nonTechQs = pickNonTechQuestions(state.config.nonTechBank, state.config.nonTechCount);

  return {
    name,
    role,
    techQuestions: techQs.slice(0, state.config.techCount),
    nonTechQuestions: nonTechQs,
    scoringContext
  };
}
