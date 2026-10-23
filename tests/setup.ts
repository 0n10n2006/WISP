import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/preact";
import { afterEach } from "vitest";

// Shared CI runners are much slower than a desktop; flows that chain a few
// async steps (record → stop → transcribe → review) can take over a second
// there. findBy*/waitFor still return as soon as the element appears.
configure({ asyncUtilTimeout: 4000 });

afterEach(() => cleanup());

if (!window.matchMedia) {
  window.matchMedia = (query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList;
}
