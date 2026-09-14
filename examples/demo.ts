// Runes TS demo: scalars, structs, containers, closures and multi-root
// merges. No tracing API is called by hand; every event comes from the
// instrumented code.
interface User {
  name: string;
  scores: number[];
  active: boolean;
}

function loadUser(): User {
  return { name: 'Ada Lovelace', scores: [90, 96, 96], active: true };
}

function double(n: number): number {
  return n * 2;
}

function normalizeName(name: string): string {
  return name.toUpperCase();
}

function totalScore(user: User): number {
  return user.scores.reduce((sum, score) => sum + score, 0);
}

function describeUser(user: User): string {
  return user.name + ' has ' + user.scores.length + ' scores';
}

rune count = 21;
const doubled = double(count);

rune user = loadUser();
const upper = normalizeName(user.name);
const total = totalScore(user);
const description = describeUser(user);

// Closure capturing a root: calls to bump carry roots=[count].
const bump = (n: number): number => n + count;
const bumped = bump(1);

// Multi-root merge: both roots flow into one call.
function combine(name: string, n: number): string {
  return name + '#' + n;
}
const combined = combine(upper, doubled);

console.log('doubled: ' + doubled);
console.log('name: ' + upper);
console.log('total score: ' + total + ' (' + (user.active ? 'active' : 'inactive') + ')');
console.log('description: ' + description);
console.log('bumped: ' + bumped);
console.log('combined: ' + combined);
