import { Marked } from "marked";
import DOMPurify from "dompurify";
import hljs from "highlight.js/lib/common";

const marked = new Marked({
  gfm: true,
  breaks: false,
  renderer: {
    code({ text, lang }) {
      const language = lang && hljs.getLanguage(lang) ? lang : undefined;
      const html = language
        ? hljs.highlight(text, { language }).value
        : escapeHtml(text);
      return `<pre><button class="copy-code" data-copy>copiar</button><code class="hljs">${html}</code></pre>`;
    },
  },
});

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function renderMarkdown(src: string): string {
  const html = marked.parse(src, { async: false }) as string;
  return DOMPurify.sanitize(html, { ADD_ATTR: ["data-copy"] });
}

export function highlight(code: string, path?: string): string {
  const ext = path?.split(".").pop()?.toLowerCase();
  const lang = ext && hljs.getLanguage(ext) ? ext : undefined;
  return lang ? hljs.highlight(code, { language: lang }).value : escapeHtml(code);
}
