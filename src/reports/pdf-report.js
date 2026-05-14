import { state } from '../app/state.js';
import { ENV } from '../app/env.js';

export function downloadPDF() {
  const { jsPDF } = window.jspdf;
  const doc = new jsPDF();
  const r = state.session.results;
  const passed = r.overall_percentage >= state.config.passThreshold;
  let y = 20;
  const margin = 15;
  const pageWidth = 210;
  const maxWidth = pageWidth - 2*margin;

  function addText(text, size, bold, color) {
    doc.setFontSize(size);
    doc.setFont('helvetica', bold ? 'bold' : 'normal');
    if (color) doc.setTextColor(...color); else doc.setTextColor(40,40,40);
    const lines = doc.splitTextToSize(text, maxWidth);
    lines.forEach(line => {
      if (y > 275) { doc.addPage(); y = 20; }
      doc.text(line, margin, y);
      y += size * 0.5;
    });
    y += 2;
  }

  function addSection(title, items, color) {
    addText(title, 13, true, color);
    items.forEach(it => addText('• ' + it, 10, false));
    y += 3;
  }

  // Header
  doc.setFillColor(79, 70, 229);
  doc.rect(0, 0, pageWidth, 30, 'F');
  doc.setTextColor(255,255,255);
  doc.setFontSize(18);
  doc.setFont('helvetica', 'bold');
  doc.text('Mock Interview Report', margin, 13);
  doc.setFontSize(11);
  doc.setFont('helvetica', 'normal');
  doc.text(`${state.session.candidateName} — ${state.session.role}`, margin, 21);
  doc.setFontSize(9);
  doc.text(`Date: ${new Date().toLocaleDateString()}`, margin, 27);
  y = 40;

  // Score
  addText(`Overall Score: ${Math.round(r.overall_percentage)}%`, 22, true,
    passed ? [16,185,129] : [239,68,68]);
  addText(passed ? `PASSED — Above ${state.config.passThreshold}% threshold. Advance to human round.`
    : `Below ${state.config.passThreshold}% threshold. Practice and retry.`, 11, true,
    passed ? [16,185,129] : [239,68,68]);
  y += 2;
  addText(`Technical: ${Math.round(r.technical_score)}%   |   Behavioral: ${Math.round(r.behavioral_score)}%`, 11, false);
  y += 4;

  // Summary
  addText('Summary', 14, true, [79,70,229]);
  addText(r.summary, 10, false);
  y += 3;

  // Strengths / Weaknesses / Actions
  addSection('Strengths', r.strengths, [16,185,129]);
  addSection('Weaknesses', r.weaknesses, [217,119,6]);
  addSection('Action Items for Next Round', r.action_items, [79,70,229]);

  // Per-question
  if (y > 230) { doc.addPage(); y = 20; }
  addText('Per-Question Breakdown', 14, true, [79,70,229]);
  r.per_question.forEach((pq, i) => {
    if (y > 250) { doc.addPage(); y = 20; }
    const q = state.session.allQuestions[i];
    addText(`Q${pq.q} (${pq.section}) — ${Math.round(pq.score)}%`, 11, true);
    addText('Question: ' + (q?.question || ''), 9, false);
    addText('Answer: ' + (q?.answer || '(no answer)').substring(0, 400), 9, false, [100,100,100]);
    addText('Feedback: ' + pq.feedback, 9, false);
    y += 2;
  });

  doc.save(`${ENV.PDF_REPORT_FILENAME_PREFIX}${state.session.candidateName.replace(/\s+/g, '_')}_${new Date().toISOString().split('T')[0]}.pdf`);
}
