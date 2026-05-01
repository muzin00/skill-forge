exports.inputs = {
  type: "object",
  properties: {
    name: { type: "string", description: "subject name" },
    count: { type: "integer", minimum: 0 },
  },
  required: ["name"],
};

exports.output = {
  type: "object",
  properties: {
    greeting: { type: "string" },
    repeated: { type: "array", items: { type: "string" } },
  },
  required: ["greeting", "repeated"],
};

exports.run = function run(args) {
  const greeting = `hello, ${args.name}`;
  const repeated = Array.from({ length: args.count ?? 0 }, () => greeting);
  return { greeting, repeated };
};
