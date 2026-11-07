import { LOCAL } from "./api";
import DOMPurify from "dompurify";

/** 站点根地址：正文里的相对路径要补全 */
const SITE = "https://huangguoai.com";

/** 把各种写法的图片地址补成绝对地址；占位符（data:）返回空串 */
function abs(raw: string): string {
  const s = (raw || "").trim();
  if (!s) return "";
  try {
    const url = new URL(s, SITE);
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) return "";
    return url.href;
  } catch { return ""; }
}

/** 把正文里的图片换成走本地协议（带 Referer / 解密能力） */
export function proxyImages(html: string): string {
  if (!html) return "";
  const doc = new DOMParser().parseFromString(html, "text/html");
  doc.querySelectorAll("img").forEach((img) => {
    // 懒加载图：真实地址常在 data-* 上，src 只是占位
    const raw =
      img.getAttribute("data-src") ||
      img.getAttribute("data-original") ||
      img.getAttribute("data-lazy-src") ||
      img.getAttribute("src") ||
      "";
    const src = abs(raw);
    if (!src) {
      img.remove();
      return;
    }
    img.setAttribute("src", `${LOCAL}/img?u=${encodeURIComponent(src)}`);
    img.removeAttribute("srcset");
    img.removeAttribute("data-src");
    img.removeAttribute("data-original");
    img.removeAttribute("data-lazy-src");
    img.removeAttribute("loading");
    img.setAttribute("loading", "lazy");
  });
  // 正文链接补全，避免点开指向本地
  doc.querySelectorAll("a[href]").forEach((a) => {
    const href = (a.getAttribute("href") || "").trim();
    const full = abs(href);
    if (full) {
      a.setAttribute("href", full);
      a.setAttribute("target", "_blank");
      a.setAttribute("rel", "noopener noreferrer");
    } else a.removeAttribute("href");
  });
  return doc.body.innerHTML;
}

/** 正文只接受所需 HTML 排版，不允许 SVG/MathML、事件、样式和可执行 URL。 */
export function sanitizeHtml(html: string): string {
  // 不支持清洗器的旧 WebView 必须拒绝正文，不能回退为原样渲染。
  if (!html || !DOMPurify.isSupported) return "";
  const clean = DOMPurify.sanitize(html, {
    ALLOWED_TAGS: ["p", "div", "span", "br", "hr", "h1", "h2", "h3", "h4", "h5", "h6",
      "ul", "ol", "li", "blockquote", "strong", "b", "em", "i", "u", "s", "del", "pre", "code",
      "a", "img", "figure", "figcaption", "table", "thead", "tbody", "tr", "th", "td"],
    ALLOWED_ATTR: ["href", "src", "alt", "title", "colspan", "rowspan", "data-src", "data-original", "data-lazy-src"],
    ALLOW_DATA_ATTR: false,
    ALLOW_ARIA_ATTR: false,
    FORBID_TAGS: ["svg", "math"],
  });
  return proxyImages(clean);
}
