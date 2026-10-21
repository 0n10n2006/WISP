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

import { encodeWav } from "../../src/services/recorder";

describe("encodeWav", () => {
  it("writes a valid 16-bit mono PCM header", () => {
    const wav = encodeWav(new Float32Array([0, 1, -1, 0.5]), 16000);
    const v = new DataView(wav.buffer);
    const tag = (o: number) => String.fromCharCode(...wav.slice(o, o + 4));
    expect(tag(0)).toBe("RIFF");
    expect(tag(8)).toBe("WAVE");
    expect(v.getUint16(22, true)).toBe(1);
    expect(v.getUint32(24, true)).toBe(16000);
    expect(v.getUint32(40, true)).toBe(8);
    expect(v.getInt16(46, true)).toBe(32767);
    expect(v.getInt16(48, true)).toBe(-32768);
    expect(wav.length).toBe(52);
  });
});
