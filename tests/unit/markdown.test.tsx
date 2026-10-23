import { render, screen } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { Markdown, parseBlocks } from "../../src/components/Markdown";

describe("parseBlocks", () => {
  it("parses common block types", () => {
    const b = parseBlocks(
      "# Title\n\npara one\nline two\n\n- a\n- b\n\n1. x\n2. y\n\n> quote\n\n```js\ncode()\n```",
    );
    expect(b.map((x) => x.type)).toEqual(["h", "p", "ul", "ol", "quote", "code"]);
    expect(b[5]).toEqual({ type: "code", lang: "js", text: "code()" });
  });

  it("handles an unterminated code fence while streaming", () => {
    const b = parseBlocks("```\npartial");
    expect(b).toEqual([{ type: "code", lang: "", text: "partial" }]);
  });
});

describe("Markdown", () => {
  it("renders inline formatting without injecting HTML", () => {
    const { container } = render(
      <Markdown text={"**bold** `code` <img src=x onerror=alert(1)>"} onLink={() => {}} />,
    );
    expect(container.querySelector("strong")?.textContent).toBe("bold");
    expect(container.querySelector("code")?.textContent).toBe("code");
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toContain("<img src=x");
  });

  it("only links http(s) URLs and routes clicks through the host", async () => {
    const onLink = vi.fn();
    render(
      <Markdown text={"[safe](https://example.com) [bad](javascript:alert(1))"} onLink={onLink} />,
    );
    const links = screen.getAllByRole("link");
    expect(links).toHaveLength(1);
    await userEvent.click(links[0]!);
    expect(onLink).toHaveBeenCalledWith("https://example.com");
  });
});

describe("links", () => {
  it("show where they really go", () => {
    render(
      <Markdown
        text="See [paypal.com](https://evil.example/login) or [example.com](https://www.example.com/docs)."
        onLink={() => {}}
      />,
    );
    expect(screen.getByRole("link", { name: /paypal\.com/ })).toHaveTextContent(
      "paypal.com (evil.example)",
    );
    expect(screen.getByRole("link", { name: /example\.com/ })).toHaveTextContent(/^example\.com$/);
  });
});
