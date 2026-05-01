exports.inputs = {
  type: "object",
  properties: {
    values: { type: "array", items: { type: "integer" } },
  },
  required: ["values"],
};

exports.output = {
  type: "object",
  properties: { sum: { type: "integer" } },
  required: ["sum"],
};

exports.run = function run(args) {
  const sum = args.values.reduce((acc, v) => acc + v, 0);
  return { sum };
};
