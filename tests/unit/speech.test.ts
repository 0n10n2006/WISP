import { toSpeakable } from "../../src/services/speech";

describe("toSpeakable", () => {
  it("removes markdown and code", () => {
    expect(
      toSpeakable(
        "# Title\n\n**Bold** and `code` with [a link](https://x.y).\n\n```js\nx()\n```\n- item",
      ),
    ).toBe("Title. Bold and code with a link. (code omitted) item");
  });
});
