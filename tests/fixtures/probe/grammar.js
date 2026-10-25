module.exports = grammar({
  name: 'probe',
  rules: {
    source: $ => repeat($.word),
    word: _ => /[a-z]+/,
  },
});
