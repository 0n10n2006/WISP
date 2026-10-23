// A deliberately small Markdown renderer that builds React elements directly
// (no HTML injection). Model output is untrusted, so links only open http(s)
// URLs through the host, which validates them again.

import { Fragment, type ReactNode } from "react";
import { Icon } from "./Icon";

export type Block =
  | { type: "p"; text: string }
  | { type: "h"; level: 1 | 2 | 3; text: string }
  | { type: "code"; lang: string; text: string }
  | { type: "ul" | "ol"; items: string[] }
  | { type: "quote"; text: string };

export function parseBlocks(src: string): Block[] {
  const lines = src.replace(/\r\n/g, "\n").split("\n");
  const blocks: Block[] = [];
  let i = 0;
  let para: string[] = [];
  const flush = () => {
    if (para.length) blocks.push({ type: "p", text: para.join("\n") });
    para = [];
  };
  while (i < lines.length) {
    const line = lines[i]!;
    const fence = line.match(/^\s*```\s*([\w+-]*)\s*$/);
    if (fence) {
      flush();
      const body: string[] = [];
      i++;
      while (i < lines.length && !/^\s*```\s*$/.test(lines[i]!)) body.push(lines[i++]!);
      i++; // closing fence (or end of a still-streaming block)
      blocks.push({ type: "code", lang: fence[1] ?? "", text: body.join("\n") });
      continue;
    }
    const h = line.match(/^(#{1,3})\s+(.*)$/);
    if (h) {
      flush();
      blocks.push({ type: "h", level: h[1]!.length as 1 | 2 | 3, text: h[2]! });
      i++;
      continue;
    }
    const ul = /^\s*[-*•]\s+(.*)$/;
    const ol = /^\s*\d+[.)]\s+(.*)$/;
    if (ul.test(line) || ol.test(line)) {
      flush();
      const type = ul.test(line) ? "ul" : "ol";
      const re = type === "ul" ? ul : ol;
      const items: string[] = [];
      while (i < lines.length && re.test(lines[i]!)) items.push(lines[i++]!.match(re)![1]!);
      blocks.push({ type, items });
      continue;
    }
    if (/^\s*>\s?/.test(line)) {
      flush();
      const q: string[] = [];
      while (i < lines.length && /^\s*>\s?/.test(lines[i]!))
        q.push(lines[i++]!.replace(/^\s*>\s?/, ""));
      blocks.push({ type: "quote", text: q.join("\n") });
      continue;
    }
    if (!line.trim()) {
      flush();
      i++;
      continue;
    }
    para.push(line);
    i++;
  }
  flush();
  return blocks;
}

const INLINE = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*[^*\s][^*]*\*)|(\[[^\]]+\]\((https?:\/\/[^\s)]+)\))/g;

/** "https://www.example.com/a" → "example.com", so a link's real
 * destination is visible next to whatever its label says. */
export function linkHost(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

export function renderInline(text: string, onLink: (url: string) => void): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  let k = 0;
  for (const m of text.matchAll(INLINE)) {
    const idx = m.index ?? 0;
    if (idx > last) out.push(text.slice(last, idx));
    const tok = m[0];
    if (m[1]) out.push(<code key={k++}>{tok.slice(1, -1)}</code>);
    else if (m[2]) out.push(<strong key={k++}>{tok.slice(2, -2)}</strong>);
    else if (m[3]) out.push(<em key={k++}>{tok.slice(1, -1)}</em>);
    else if (m[4]) {
      const label = tok.slice(1, tok.indexOf("]("));
      const url = m[5]!;
      const host = linkHost(url);
      out.push(
        <a
          key={k++}
          href={url}
          onClick={(e) => {
            e.preventDefault();
            onLink(url);
          }}
          onAuxClick={(e) => e.preventDefault()}
          onDragStart={(e) => e.preventDefault()}
          title={url}
        >
          {label}
          {host && !label.toLowerCase().includes(host) && (
            <span className="md-link-host"> ({host})</span>
          )}
        </a>,
      );
    }
    last = idx + tok.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

function withBreaks(nodes: ReactNode[]): ReactNode[] {
  return nodes.flatMap((n, i): ReactNode[] =>
    typeof n === "string"
      ? n.split("\n").map((part, j, arr) => (
          <Fragment key={`${i}-${j}`}>
            {part}
            {j < arr.length - 1 && <br />}
          </Fragment>
        ))
      : [n],
  );
}

export function Markdown({
  text,
  onLink,
  onCopy,
}: {
  text: string;
  onLink: (url: string) => void;
  onCopy?: (code: string) => void;
}) {
  const blocks = parseBlocks(text);
  return (
    <div className="md">
      {blocks.map((b, i) => {
        switch (b.type) {
          case "p":
            return <p key={i}>{withBreaks(renderInline(b.text, onLink))}</p>;
          case "h": {
            const Tag = `h${b.level + 2}` as "h3" | "h4" | "h5";
            return <Tag key={i}>{renderInline(b.text, onLink)}</Tag>;
          }
          case "code":
            return (
              <div key={i} className="code-block">
                <pre data-lang={b.lang || undefined}>
                  <code>{b.text}</code>
                </pre>
                {onCopy && (
                  <button
                    className="icon-btn icon-btn-small code-copy"
                    onClick={() => onCopy(b.text)}
                    aria-label="Copy code"
                    title="Copy code"
                  >
                    <Icon name="copy" size={14} />
                  </button>
                )}
              </div>
            );
          case "ul":
            return (
              <ul key={i}>
                {b.items.map((it, j) => (
                  <li key={j}>{renderInline(it, onLink)}</li>
                ))}
              </ul>
            );
          case "ol":
            return (
              <ol key={i}>
                {b.items.map((it, j) => (
                  <li key={j}>{renderInline(it, onLink)}</li>
                ))}
              </ol>
            );
          case "quote":
            return <blockquote key={i}>{withBreaks(renderInline(b.text, onLink))}</blockquote>;
        }
      })}
    </div>
  );
}
