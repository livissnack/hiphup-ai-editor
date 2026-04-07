import DOMPurify from 'dompurify';
import type { Config as DompurifyConfig } from 'dompurify';
import { marked, Renderer } from 'marked';
import type { Tokens } from 'marked';

function escapeHtml(raw: string): string {
  return raw
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function escapeAttr(raw: string): string {
  return escapeHtml(raw).replace(/'/g, '&#39;');
}

const previewRenderer = new Renderer();

previewRenderer.code = function (this: Renderer, token: Tokens.Code): string {
  const lang = token.lang?.match(/^[\w-]+$/)?.[0] ?? '';
  const codeClass = lang ? ` class="language-${lang}"` : '';
  const body = token.escaped ? token.text : escapeHtml(token.text);
  return `<pre class="md-pre"><code${codeClass}>${body}</code></pre>`;
};

previewRenderer.link = function (this: Renderer, token: Tokens.Link): string {
  const text = this.parser.parseInline(token.tokens);
  const href = token.href ?? '';
  const titlePart =
    token.title != null && token.title !== ''
      ? ` title="${escapeAttr(String(token.title))}"`
      : '';
  return `<a href="${escapeAttr(href)}" class="md-link" target="_blank" rel="noopener noreferrer"${titlePart}>${text}</a>`;
};

previewRenderer.image = function (token: Tokens.Image): string {
  const alt = escapeAttr(token.text);
  const titlePart =
    token.title != null && token.title !== ''
      ? ` title="${escapeAttr(String(token.title))}"`
      : '';
  return `<img src="${escapeAttr(token.href)}" alt="${alt}" class="md-img" loading="lazy"${titlePart} />`;
};

marked.use({
  renderer: previewRenderer,
  gfm: true,
  breaks: false,
});

const PURIFY: DompurifyConfig = {
  ADD_TAGS: ['input'],
  ADD_ATTR: ['target', 'rel', 'checked', 'disabled', 'class', 'loading', 'align', 'type'],
};

export function renderMarkdownPreview(markdown: string): string {
  const html = marked.parse(markdown, { async: false }) as string;
  // DOMPurify may return TrustedHTML when Trusted Types are enabled; our app expects a string.
  return DOMPurify.sanitize(html, PURIFY) as unknown as string;
}
