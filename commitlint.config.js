export default {
  extends: ["@commitlint/config-conventional"],
  rules: {
    "type-case": [2, "always", "lower-case"],
    "scope-case": [2, "always", "lower-case"],
    "subject-max-length": [2, "always", 50],
    "body-full-stop": [0],
    "body-full-stop-without-footer": [2, "always", "."],
    "body-leading-blank": [2, "always"],
    "body-max-line-length": [2, "always", 72],
    "body-case": [2, "always", "sentence-case"],
    "footer-leading-blank": [2, "always"],
    "footer-max-line-length": [2, "always", 72],
  },

  plugins: [
    {
      rules: {
        "body-full-stop-without-footer": (
          parsed,
          when = "always",
          value = ".",
        ) => {
          if (!parsed.body) {
            return [true];
          }

          const body = removeFooters(parsed.body).trim();

          if (!body) {
            return [true];
          }

          const hasFullStop = body.endsWith(value);
          const valid = when === "always" ? hasFullStop : !hasFullStop;

          return [
            valid,
            `body must ${when === "always" ? "" : "not "}end with "${value}" excluding footer`,
          ];
        },
      },
    },
  ],
};

function removeFooters(body) {
  const lines = body.split(/\r?\n/);
  let end = lines.length;

  while (end > 0) {
    const line = lines[end - 1];

    if (line.trim() === "" || isFooterLine(line)) {
      end -= 1;
      continue;
    }

    break;
  }

  return lines.slice(0, end).join("\n");
}

function isFooterLine(line) {
  return /^(BREAKING CHANGE|[A-Za-z0-9-]+)(: | #).+/.test(line);
}
