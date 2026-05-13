# AI Mock Interview Platform

A self-contained, single-file web app that runs verbal mock interviews for candidates using Google Gemini. Built for HR teams who want a zero-friction screening step before live human interviews.

## What it does

HR pastes a job description and the candidate's resume. The app generates a tailored set of technical questions (focused on the JD) plus categorized behavioral questions. The candidate gets a personal link, opens it, and a voice-based interview begins — questions are spoken aloud in a US accent, answers are transcribed via the browser's microphone. At the end the candidate sees a detailed report (strengths, weaknesses, action items, per-question scoring, overall percentage) and can download it as PDF. HR automatically receives a copy by email.

## Tech stack

- Single HTML file. No build step, no backend.
- Google Gemini API for question generation and scoring (free tier covers ~100 interviews/day).
- Web Speech API for voice in/out (browser-native).
- PDF.js for resume PDF parsing.
- jsPDF for report generation.
- Netlify Forms for emailing reports.

## Setup

1. Get a free Gemini API key at [aistudio.google.com/app/apikey](https://aistudio.google.com/app/apikey).
2. Open `mock-interview-app.html` in Chrome or Edge.
3. Go to **HR Configuration**, paste your API key, your email, and your behavioral question bank (one question per line under `## Category Name` headers).
4. Go to **Deploy** tab, click **Download Deployable HTML**, drag the file onto [app.netlify.com/drop](https://app.netlify.com/drop) to host it.
5. Restrict your Gemini API key by HTTP referrer (Google Cloud Console) so only your Netlify URL can use it.
6. In your Netlify site dashboard, set up email notifications for the `interview-result` form.

## Generating a candidate interview

1. Open your hosted URL.
2. Go to **Generate Candidate Link**, paste JD + resume (or upload PDF), enter name and role.
3. Click Generate — the AI pre-builds questions in ~10 seconds.
4. Copy the link and send to the candidate.

## Candidate experience

Candidate clicks the link, clicks Start, voice-based interview begins. No setup. No accounts. ~20-30 minutes. They get the report immediately; HR gets a copy by email.

## Browser support

Use Chrome or Edge. Voice transcription is unreliable in Safari and Firefox.

## License

Private — for internal HR use.
