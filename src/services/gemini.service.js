import { state } from '../app/state.js';
import { ENV, envNumber } from '../app/env.js';

export async function callAI(systemPrompt, userMessage, maxTokens = envNumber('GEMINI_DEFAULT_MAX_OUTPUT_TOKENS'), responseSchema = null, useThinking = false) {
  const model = ENV.GEMINI_MODEL;
  const url = `${ENV.GEMINI_GENERATE_CONTENT_BASE_URL}/${model}:${ENV.GEMINI_GENERATE_CONTENT_METHOD}?key=${encodeURIComponent(state.config.apiKey)}`;
  const generationConfig = {
    maxOutputTokens: maxTokens,
    temperature: envNumber('GEMINI_TEMPERATURE'),
    responseMimeType: ENV.GEMINI_RESPONSE_MIME_TYPE
  };
  if (responseSchema) generationConfig.responseSchema = responseSchema;
  // Disable thinking for non-reasoning tasks (saves tokens, faster, cheaper)
  if (!useThinking) {
    generationConfig.thinkingConfig = { thinkingBudget: envNumber('GEMINI_THINKING_BUDGET') };
  }

  const resp = await fetch(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      systemInstruction: { parts: [{ text: systemPrompt }] },
      contents: [{ role: 'user', parts: [{ text: userMessage }] }],
      generationConfig
    })
  });
  if (!resp.ok) {
    const err = await resp.text();
    throw new Error('API error: ' + err);
  }
  const data = await resp.json();
  const text = data?.candidates?.[0]?.content?.parts?.[0]?.text;
  const finishReason = data?.candidates?.[0]?.finishReason;
  if (!text) {
    throw new Error('Empty response from Gemini (reason: ' + finishReason + '): ' + JSON.stringify(data).slice(0, 400));
  }
  if (finishReason === 'MAX_TOKENS') {
    console.warn('Response hit max tokens; may be truncated. Length:', text.length);
  }
  return text;
}
