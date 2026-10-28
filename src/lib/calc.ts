// The bar's calculator and unit converter. A small hand-written parser:
// nothing typed is ever evaluated as code.

export interface CalcResult {
  /** The answer as shown ("1,234.5", "8.05 mi"). */
  display: string;
  /** What Enter copies (no thousands separators). */
  copy: string;
  /** "Calculator", "Length", "Currency · rates of 2026-10-27"… */
  hint: string;
}

/** Exchange rates against one base, from the host (ECB data). */
export interface Rates {
  base: string;
  date: string;
  rates: Record<string, number>;
}

// ---------------------------------------------------------------------------
// Expressions

type Tok =
  | { t: "num"; v: number }
  | { t: "op"; v: string }
  | { t: "id"; v: string }
  | { t: "("; v: "(" }
  | { t: ")"; v: ")" };

const FUNCS: Record<string, (x: number) => number> = {
  sqrt: Math.sqrt,
  sin: (x) => Math.sin(x),
  cos: (x) => Math.cos(x),
  tan: (x) => Math.tan(x),
  log: Math.log10,
  ln: Math.log,
  abs: Math.abs,
  round: Math.round,
  floor: Math.floor,
  ceil: Math.ceil,
};
const CONSTS: Record<string, number> = { pi: Math.PI, e: Math.E };

function tokenize(src: string): Tok[] | null {
  const out: Tok[] = [];
  let i = 0;
  const s = src.replace(/×/g, "*").replace(/÷/g, "/").replace(/\*\*/g, "^");
  while (i < s.length) {
    const c = s[i]!;
    if (c === " ") {
      i++;
      continue;
    }
    const num = s.slice(i).match(/^(\d{1,3}(?:,\d{3})+|\d+)?(\.\d+)?/);
    if (num && num[0]) {
      out.push({ t: "num", v: Number(num[0].replace(/,/g, "")) });
      i += num[0].length;
      continue;
    }
    if ("+-*/^%".includes(c)) {
      out.push({ t: "op", v: c });
      i++;
      continue;
    }
    if (c === "(" || c === ")") {
      out.push(c === "(" ? { t: "(", v: "(" } : { t: ")", v: ")" });
      i++;
      continue;
    }
    const id = s.slice(i).match(/^[a-z]+/i);
    if (id) {
      out.push({ t: "id", v: id[0].toLowerCase() });
      i += id[0].length;
      continue;
    }
    return null;
  }
  return out;
}

/** Recursive descent: expr = term (+|- term)*; term = power (*|/ power)*; power = unary (^ power)? */
function parse(toks: Tok[]): number | null {
  let p = 0;
  const peek = () => toks[p];
  const fail = (): never => {
    throw new Error("bad");
  };
  const expr = (): number => {
    let v = term();
    for (;;) {
      const t = peek();
      if (t?.t === "op" && (t.v === "+" || t.v === "-")) {
        p++;
        const r = term();
        v = t.v === "+" ? v + r : v - r;
      } else return v;
    }
  };
  const term = (): number => {
    let v = unary();
    for (;;) {
      const t = peek();
      if (t?.t === "op" && (t.v === "*" || t.v === "/")) {
        p++;
        const r = unary();
        v = t.v === "*" ? v * r : v / r;
      } else if (t && (t.t === "(" || t.t === "num" || t.t === "id")) {
        // Implicit multiplication: 2(3+4), 2pi.
        v *= unary();
      } else return v;
    }
  };
  // -3^2 is -(3^2), as on paper.
  const unary = (): number => {
    const t = peek();
    if (t?.t === "op" && (t.v === "-" || t.v === "+")) {
      p++;
      const v = unary();
      return t.v === "-" ? -v : v;
    }
    return power();
  };
  const power = (): number => {
    const base = postfix();
    const t = peek();
    if (t?.t === "op" && t.v === "^") {
      p++;
      return base ** unary();
    }
    return base;
  };
  const postfix = (): number => {
    let v = atom();
    const t = peek();
    if (t?.t === "op" && t.v === "%") {
      p++;
      v /= 100;
    }
    return v;
  };
  const atom = (): number => {
    const t = toks[p++];
    if (!t) return fail();
    if (t.t === "num") return t.v;
    if (t.t === "(") {
      const v = expr();
      if (toks[p++]?.t !== ")") fail();
      return v;
    }
    if (t.t === "id") {
      if (t.v in CONSTS) return CONSTS[t.v]!;
      const f = FUNCS[t.v];
      if (f) {
        if (toks[p]?.t !== "(") fail();
        p++;
        const v = expr();
        if (toks[p++]?.t !== ")") fail();
        return f(v);
      }
    }
    return fail();
  };
  try {
    const v = expr();
    return p === toks.length ? v : null;
  } catch {
    return null;
  }
}

/** Evaluate an arithmetic expression, or null if it isn't one. */
export function evaluate(src: string): number | null {
  const toks = tokenize(src.trim());
  if (!toks || !toks.length) return null;
  const v = parse(toks);
  return v === null || !Number.isFinite(v) ? null : v;
}

/** 1234.5 → "1,234.5"; very large/small numbers in exponent form. */
export function formatNumber(v: number): string {
  if (v !== 0 && (Math.abs(v) >= 1e15 || Math.abs(v) < 1e-6)) return v.toExponential(6);
  const rounded = Math.round(v * 1e10) / 1e10;
  return rounded.toLocaleString("en-US", { maximumFractionDigits: 10 });
}

function plain(v: number): string {
  if (v !== 0 && (Math.abs(v) >= 1e15 || Math.abs(v) < 1e-6)) return v.toExponential(6);
  return String(Math.round(v * 1e10) / 1e10);
}

// ---------------------------------------------------------------------------
// Units

interface Unit {
  dim: string;
  /** Multiply to get the base unit (ignored for temperature). */
  f: number;
  label: string;
}

const U: Record<string, Unit> = {};
function def(dim: string, f: number, label: string, ...names: string[]) {
  for (const n of [label, ...names]) U[n.toLowerCase()] = { dim, f, label };
}
// Length (metres)
def("Length", 1e-3, "mm", "millimetre", "millimetres", "millimeter", "millimeters");
def("Length", 1e-2, "cm", "centimetre", "centimetres", "centimeter", "centimeters");
def("Length", 1, "m", "metre", "metres", "meter", "meters");
def("Length", 1e3, "km", "kilometre", "kilometres", "kilometer", "kilometers", "kms");
def("Length", 0.0254, "in", "inch", "inches");
def("Length", 0.3048, "ft", "foot", "feet");
def("Length", 0.9144, "yd", "yard", "yards");
def("Length", 1609.344, "mi", "mile", "miles");
def("Length", 1852, "nmi", "nautical mile", "nautical miles");
// Mass (grams)
def("Mass", 1e-3, "mg", "milligram", "milligrams");
def("Mass", 1, "g", "gram", "grams");
def("Mass", 1e3, "kg", "kilogram", "kilograms", "kilo", "kilos");
def("Mass", 1e6, "t", "tonne", "tonnes");
def("Mass", 28.349523125, "oz", "ounce", "ounces");
def("Mass", 453.59237, "lb", "lbs", "pound", "pounds");
def("Mass", 6350.29318, "st", "stone", "stones");
// Volume (litres)
def("Volume", 1e-3, "ml", "millilitre", "millilitres", "milliliter", "milliliters");
def("Volume", 1, "l", "litre", "litres", "liter", "liters");
def("Volume", 3.785411784, "gal", "gallon", "gallons", "us gallon");
def("Volume", 4.54609, "uk gal", "imperial gallon", "imperial gallons");
def("Volume", 0.0295735295625, "fl oz", "fluid ounce", "fluid ounces");
def("Volume", 0.2365882365, "cup", "cups");
def("Volume", 0.473176473, "pt", "pint", "pints");
// Speed (m/s)
def("Speed", 1, "m/s");
def("Speed", 1 / 3.6, "km/h", "kph", "kmh");
def("Speed", 0.44704, "mph");
def("Speed", 0.514444, "kn", "knot", "knots");
// Area (m²)
def("Area", 1, "m2", "sq m", "square metre", "square metres", "square meters");
def("Area", 1e6, "km2", "sq km", "square kilometres", "square kilometers");
def("Area", 0.09290304, "ft2", "sq ft", "square foot", "square feet");
def("Area", 4046.8564224, "acre", "acres");
def("Area", 1e4, "ha", "hectare", "hectares");
// Data (bytes)
def("Data", 1, "B", "byte", "bytes");
def("Data", 1e3, "KB", "kilobyte", "kilobytes");
def("Data", 1e6, "MB", "megabyte", "megabytes");
def("Data", 1e9, "GB", "gigabyte", "gigabytes");
def("Data", 1e12, "TB", "terabyte", "terabytes");
def("Data", 1024, "KiB");
def("Data", 1024 ** 2, "MiB");
def("Data", 1024 ** 3, "GiB");
def("Data", 1024 ** 4, "TiB");
// Time (seconds)
def("Time", 1e-3, "ms", "millisecond", "milliseconds");
def("Time", 1, "s", "sec", "secs", "second", "seconds");
def("Time", 60, "min", "mins", "minute", "minutes");
def("Time", 3600, "h", "hr", "hrs", "hour", "hours");
def("Time", 86400, "d", "day", "days");
def("Time", 604800, "wk", "week", "weeks");
def("Time", 31557600, "yr", "year", "years");
// Temperature
def("Temperature", 0, "°C", "c", "celsius", "°c", "degc");
def("Temperature", 0, "°F", "f", "fahrenheit", "°f", "degf");
def("Temperature", 0, "K", "k", "kelvin");

function toCelsius(v: number, u: string): number {
  return u === "°F" ? ((v - 32) * 5) / 9 : u === "K" ? v - 273.15 : v;
}
function fromCelsius(c: number, u: string): number {
  return u === "°F" ? (c * 9) / 5 + 32 : u === "K" ? c + 273.15 : c;
}

const CONVERT = /^(.+?)\s*([a-z°/²][a-z°/² ]*?)\s+(?:in|to|as|into|=)\s+([a-z°/²][a-z°/² ]*)$/i;

function unit(name: string): Unit | undefined {
  const n = name.trim().replace(/²/g, "2");
  return U[n] ?? U[n.toLowerCase()];
}

/** Currency codes Wisp knows how to convert (the ECB list). */
const CURRENCY_WORDS: Record<string, string> = {
  dollar: "USD",
  dollars: "USD",
  euro: "EUR",
  euros: "EUR",
  pound: "GBP",
  pounds: "GBP",
  quid: "GBP",
  yen: "JPY",
  rupee: "INR",
  rupees: "INR",
  $: "USD",
  "€": "EUR",
  "£": "GBP",
  "¥": "JPY",
};

function currency(code: string, rates: Rates | null): string | null {
  const c = CURRENCY_WORDS[code.toLowerCase()] ?? code.toUpperCase();
  if (!/^[A-Z]{3}$/.test(c)) return null;
  if (!rates) return c;
  return c === rates.base || c in rates.rates ? c : null;
}

function convertCurrency(amount: number, from: string, to: string, rates: Rates): number | null {
  const r = (c: string) => (c === rates.base ? 1 : rates.rates[c]);
  const a = r(from);
  const b = r(to);
  return a && b ? (amount / a) * b : null;
}

/**
 * Answer `query` as arithmetic or a conversion; null if it's neither.
 * A plain number ("42") isn't an answer.
 */
export function calculate(query: string, rates: Rates | null = null): CalcResult | null {
  let q = query.trim().replace(/^=\s*/, "").replace(/[?]$/, "");
  if (!q || q.length > 200) return null;
  q = q.replace(/^(?:what(?:'s| is)|calculate|convert)\s+/i, "");

  // "12% of 80"
  const pct = q.match(/^(.+?)\s*%\s*of\s+(.+)$/i);
  if (pct) {
    const a = evaluate(pct[1]!);
    const b = evaluate(pct[2]!);
    if (a !== null && b !== null) {
      const v = (a / 100) * b;
      return { display: formatNumber(v), copy: plain(v), hint: "Calculator" };
    }
  }

  // Currency: "30 usd to eur", "$30 in gbp"
  const cur = q.match(
    /^([$€£¥])?\s*([\d.,]+(?:\s*[-+*/^]\s*[\d.,]+)*)\s*([a-z]{3}|[a-z]+)?\s+(?:in|to|as|into)\s+([a-z]{3}|[a-z]+|[$€£¥])$/i,
  );
  if (cur) {
    const amount = evaluate(cur[2]!);
    const from = currency(cur[1] ?? cur[3] ?? "", rates);
    const to = currency(cur[4]!, rates);
    if (amount !== null && from && to && !unit(cur[3] ?? "") && !unit(cur[4]!)) {
      if (!rates) return { display: "…", copy: "", hint: "Currency · getting today's rates" };
      const v = convertCurrency(amount, from, to, rates);
      if (v !== null) {
        const shown = v.toLocaleString("en-US", {
          maximumFractionDigits: 2,
          minimumFractionDigits: 2,
        });
        return {
          display: `${shown} ${to}`,
          copy: (Math.round(v * 100) / 100).toFixed(2),
          hint: `${formatNumber(amount)} ${from} · rates of ${rates.date}`,
        };
      }
    }
  }

  // Units: "5 km in miles", "72f to c", "1.5 gb in mib"
  const conv = q.match(CONVERT);
  if (conv) {
    const amount = evaluate(conv[1]!);
    const a = unit(conv[2]!);
    const b = unit(conv[3]!);
    if (amount !== null && a && b && a.dim === b.dim) {
      const v =
        a.dim === "Temperature"
          ? fromCelsius(toCelsius(amount, a.label), b.label)
          : (amount * a.f) / b.f;
      return {
        display: `${formatNumber(v)} ${b.label}`,
        copy: plain(v),
        hint: `${formatNumber(amount)} ${a.label} · ${a.dim}`,
      };
    }
  }

  // Arithmetic needs an operator or a function, so "42" or "2024" isn't one.
  if (!/[-+*/^%×÷()]|\b(sqrt|sin|cos|tan|log|ln|abs|round|floor|ceil|pi)\b/i.test(q)) return null;
  if (/^[a-z\s]+$/i.test(q) && q.toLowerCase() !== "pi") return null;
  const v = evaluate(q);
  if (v === null) return null;
  return { display: formatNumber(v), copy: plain(v), hint: "Calculator" };
}
