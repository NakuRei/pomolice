export default {
  extends: ["@commitlint/config-conventional"],
  rules: {
    "type-case": [2, "always", "lower-case"],
    "scope-case": [2, "always", "lower-case"],
    "subject-max-length": [2, "always", 50],
    "body-full-stop": [2, "always", "."],
    "body-leading-blank": [2, "always"],
    "body-max-line-length": [2, "always", 72],
    "body-case": [2, "always", "sentence-case"],
    "footer-leading-blank": [2, "always"],
    "footer-max-line-length": [2, "always", 72],
  },
};
