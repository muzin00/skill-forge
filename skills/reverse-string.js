exports.inputs = {
  type: "object",
  properties: { str: { type: "string" } },
  required: ["str"],
};

exports.output = {
  type: "object",
  properties: { reversed: { type: "string" } },
  required: ["reversed"],
};

exports.run = function run(args) {
  const reversed = Array.from(args.str).reverse().join("");
  return { reversed };
};
