#!/usr/bin/env python3
"""Writes the programming-language word lists (catalog/languages/code_*.toml):
keywords, built-ins and common idioms per language. Edit the lists below,
run `scripts/code-languages.py catalog/languages`, then regenerate the index
with `TTYP_BLESS=1 cargo test catalog_index`."""
import json, sys
L = {}
def lang(name, display, text):
    L[name] = (display, text.split())

lang("python", "Python", r"""
def return if elif else for while in not and or is None True False import from as class self
try except finally raise with yield lambda pass break continue global nonlocal assert del async await
print() len() range() enumerate() zip() map() filter() sorted() reversed() sum() min() max() abs() round()
int() float() str() bool() list() dict() set() tuple() type() isinstance() hasattr() getattr() setattr()
open() input() super() iter() next() any() all() __init__ __name__ __main__ __repr__ __str__ __eq__
self.name self.value cls @property @staticmethod @classmethod @dataclass
append() extend() insert() pop() remove() index() count() keys() values() items() get() update() copy()
split() join() strip() replace() format() startswith() endswith() lower() upper() find() encode() decode()
import os import sys import json import re from typing List Dict Optional Any Callable Iterable
os.path sys.argv json.loads json.dumps re.match f"{x}" "__main__" if __name__ == "__main__": args kwargs *args **kwargs
[x for x in xs] {k: v} ValueError TypeError KeyError IndexError Exception StopIteration
with open(path) as f: f.read() f.write() for i in range(n): return None yield from
numpy np pandas pd pytest assert raise ValueError("bad") -> int -> str x: int = 0
""")

lang("cpp", "C++", r"""
#include <iostream> <vector> <string> <map> <memory> <algorithm> <unordered_map>
int main() return 0; std::cout std::cin std::endl std::string std::vector<int> std::map std::unique_ptr std::shared_ptr std::make_unique std::move
namespace using class struct public: private: protected: virtual override final const constexpr static inline
template <typename T> auto void bool char int long double float unsigned size_t nullptr true false
new delete this operator friend explicit mutable volatile noexcept enum typedef sizeof decltype
if else for while do switch case default: break; continue; try catch throw
std::sort std::find std::begin std::end std::pair std::optional std::array std::thread std::mutex
push_back() emplace_back() size() empty() begin() end() clear() insert() erase() find() at() front() back()
const& && -> :: << >> ++i i++ != == <= >= &x *ptr ptr-> ~Foo() Foo::Foo() #pragma once #define #ifdef #endif
for (auto& x : xs) { } std::string_view static_cast<int> dynamic_cast reinterpret_cast const_cast
""")

lang("c", "C", r"""
#include <stdio.h> <stdlib.h> <string.h> <stdint.h> <stdbool.h> <assert.h>
int main(void) main(int argc, char **argv) return 0; printf() scanf() fprintf() stderr stdout stdin
malloc() calloc() realloc() free() memcpy() memset() strlen() strcpy() strcmp() strncpy() sizeof()
void char short int long float double unsigned signed const static extern volatile register
struct union enum typedef if else for while do switch case default: break; continue; goto
NULL EOF size_t uint8_t uint32_t int64_t bool true false FILE fopen() fclose() fgets() fputs() fread() fwrite()
#define #ifndef #ifdef #endif #pragma #if #else inline restrict
*ptr &x ptr->next -> ++ -- != == <= >= && || << >> %d %s %zu \n exit() abort() atoi()
for (int i = 0; i < n; i++) { } char buf[256]; struct node *next; typedef struct { } Node;
""")

lang("javascript", "JavaScript", r"""
function return const let var if else for while do switch case default: break continue
try catch finally throw new this class extends super constructor static get set async await yield
import export default from as typeof instanceof in of delete void null undefined true false NaN
console.log() console.error() JSON.parse() JSON.stringify() Object.keys() Object.values() Object.entries()
Array.isArray() Array.from() Promise.all() Promise.resolve() setTimeout() setInterval() clearTimeout()
map() filter() reduce() forEach() find() some() every() includes() indexOf() push() pop() shift()
slice() splice() concat() join() split() trim() toString() length then() catch() fetch()
document.querySelector() addEventListener() window require() module.exports process.env
=> === !== ?? ?. ... `${x}` () => {} async () => {} Math.max() Math.floor() Math.random() Date.now()
Map Set Symbol Error TypeError RegExp parseInt() parseFloat() Number String Boolean
""")

lang("typescript", "TypeScript", r"""
interface type enum namespace declare readonly abstract implements private public protected
string number boolean any unknown never void null undefined object bigint symbol
function return const let if else for while switch case break continue try catch finally throw
class extends super constructor new this static async await import export default from as
keyof typeof infer extends satisfies is asserts
Partial<T> Required<T> Readonly<T> Record<K, Pick<T, Omit<T, Exclude<T, Extract<T, ReturnType<T> NonNullable<T>
Promise<void> Array<string> Map<string, Set<number> string[] number[] ?: ! ?. ?? => ===
console.log() JSON.parse() Object.keys() map() filter() reduce() forEach() find() includes()
<T> <T extends > as const unknown[] never[] T[] export type export interface import type
@Component() @Injectable() tsconfig.json strict noImplicitAny
""")

lang("ocaml", "OCaml", r"""
let in rec and fun function match with | -> if then else begin end type of module struct sig
open include functor val mutable ref ! := when as try raise exception assert lazy
true false () unit int float string bool char list array option Some None
List.map List.iter List.fold_left List.filter List.length List.rev List.nth Array.make Array.length
String.length String.concat String.sub Printf.printf print_endline print_string print_int
Hashtbl.create Hashtbl.add Hashtbl.find Option.map Result Ok Error failwith invalid_arg
:: @ ^ ; ;; <> = == != < > <= >= +. *. /. -. |> @@ fst snd not
let () = let rec go acc = function | [] -> acc | x :: xs -> go
module M = struct end module type S = sig end dune opam utop
""")

lang("java", "Java", r"""
public private protected static final abstract class interface extends implements enum record
void int long double float boolean char byte short String Object Integer Long Boolean
return if else for while do switch case default: break; continue; try catch finally throw throws
new this super null true false instanceof import package synchronized volatile transient native var
public static void main(String[] args) System.out.println() System.err String.format() Math.max()
List<String> ArrayList<> Map<String, HashMap<> Set<Integer> HashSet<> Optional<T> Stream
@Override @Deprecated @FunctionalInterface @SuppressWarnings
add() get() put() size() isEmpty() contains() remove() equals() hashCode() toString() length()
stream() map() filter() collect() Collectors.toList() forEach() orElse() of() ofNullable()
Exception RuntimeException IllegalArgumentException NullPointerException IOException
-> :: != == && || ++ -- this.name getName() setName() Thread Runnable
""")

lang("csharp", "C#", r"""
using namespace class struct interface enum record public private protected internal static readonly
const void int long double float decimal bool char string object var dynamic
return if else for foreach in while do switch case default: break; continue; try catch finally throw
new this base null true false is as typeof sizeof nameof async await yield get; set; init;
virtual override abstract sealed partial params ref out
Console.WriteLine() Console.ReadLine() string.Format() Math.Max() DateTime.Now Guid.NewGuid()
List<int> Dictionary<string, IEnumerable<T> Task<T> Action Func<T> Span<T> IDisposable
Add() Remove() Contains() Count Length ToString() Equals() GetHashCode() ToList() ToArray()
Where() Select() OrderBy() FirstOrDefault() Any() All() Sum() GroupBy()
=> ?? ?. ! $"{x}" @"" [Serializable] [HttpGet] Exception ArgumentNullException
""")

lang("go", "Go", r"""
package main import func return var const type struct interface map chan go defer select
if else for range switch case default: break continue fallthrough goto
int int64 uint8 float64 string bool byte rune error any nil true false iota
make() new() len() cap() append() copy() delete() panic() recover() close() print()
fmt.Println() fmt.Printf() fmt.Sprintf() fmt.Errorf() errors.New() os.Exit() os.Args
strings.Split() strings.Join() strconv.Itoa() strconv.Atoi() time.Now() time.Sleep()
context.Context ctx http.HandleFunc() http.ListenAndServe() json.Marshal() json.Unmarshal()
sync.WaitGroup sync.Mutex io.Reader io.Writer bufio.NewScanner()
:= <- ... != == && || err if err != nil { return err } func (s *Server) []string map[string]int
go.mod go.sum go run go build go test t.Fatal() t.Errorf()
""")

lang("rust", "Rust", r"""
fn let mut const static struct enum impl trait pub use mod crate self Self super
match if else loop while for in return break continue move ref as where unsafe async await dyn
i32 i64 u8 u32 u64 usize f64 bool char str String Vec<T> Option<T> Result<T, Box<dyn HashMap BTreeMap
Some None Ok() Err() true false unwrap() expect() ? clone() to_string() into() iter() collect()
map() filter() fold() len() push() pop() is_empty() contains() get() insert() as_str() as_ref()
println!() format!() vec![] assert_eq!() panic!() todo!() unimplemented!() #[derive(Debug)] #[test]
&self &mut self -> => :: 'a &'a str impl<T> where T: Clone + Send + Sync Rc<RefCell<T>> Arc<Mutex<T>>
cargo build cargo test cargo run std::io std::fs std::collections serde tokio anyhow
""")

lang("php", "PHP", r"""
<?php ?> echo print function return if else elseif foreach as while for do switch case default: break;
class interface trait extends implements public private protected static final abstract new
$this $x $arr $_GET $_POST $_SERVER $_SESSION self:: parent:: namespace use require_once include
null true false array() isset() empty() unset() count() strlen() str_replace() explode() implode()
array_map() array_filter() array_keys() in_array() json_encode() json_decode() var_dump() die()
try catch finally throw Exception PDO mysqli_query() header() session_start()
=> -> :: . .= === !== ?? ?-> fn() match readonly enum string int float bool mixed void
composer.json Laravel Route::get() $request->input()
""")

lang("ruby", "Ruby", r"""
def end class module if elsif else unless case when while until for in do begin rescue ensure raise
return yield self nil true false and or not require require_relative include extend attr_accessor
attr_reader puts print p gets each map select reject reduce each_with_index times upto inject
length size empty? nil? include? push pop first last sort sort_by reverse join split strip to_s to_i to_sym
@name @@count :symbol => {} |x| do |item| end "#{x}" Hash Array String Integer Proc lambda ->
.freeze .dup .tap private protected initialize super Struct.new Comparable Enumerable
rails bundle gem Gemfile rspec describe it expect().to eq()
""")

lang("swift", "Swift", r"""
func let var return if else guard switch case default: for in while repeat break continue
class struct enum protocol extension init deinit self Self super import public private internal fileprivate open
static final lazy weak unowned mutating inout throws throw try try? catch do defer async await
Int Double Float String Bool Character Array Dictionary Set Optional Any AnyObject Void
nil true false some any where associatedtype typealias
print() count isEmpty append() remove() contains() map() filter() reduce() forEach() sorted()
?? ?. ! -> ... ..< == != "\(x)" @State @Binding @Published @MainActor @escaping @objc
if let guard let SwiftUI View var body: some View Text() VStack HStack Button()
""")

lang("kotlin", "Kotlin", r"""
fun val var return if else when for in while do break continue class object interface enum data sealed
open abstract override private public internal protected companion init constructor this super
import package null true false is as as? in !in typealias suspend inline reified lateinit by lazy
Int Long Double Float Boolean String Char Unit Any Nothing List<T> MutableList Map<K, V> Set
listOf() mutableListOf() mapOf() setOf() println() print() require() check() error() also apply let run with
map filter forEach first last size isEmpty() contains() joinToString() toString()
?. ?: !! -> :: .. $name "${x}" @JvmStatic @Composable coroutineScope launch async await() Flow
""")

lang("sql", "SQL", r"""
SELECT FROM WHERE AND OR NOT IN IS NULL LIKE BETWEEN EXISTS DISTINCT AS ON
INSERT INTO VALUES UPDATE SET DELETE CREATE TABLE ALTER ADD DROP INDEX VIEW PRIMARY KEY FOREIGN REFERENCES
JOIN LEFT RIGHT INNER OUTER FULL CROSS GROUP BY ORDER HAVING LIMIT OFFSET UNION ALL ASC DESC
COUNT(*) SUM() AVG() MIN() MAX() COALESCE() CAST() CASE WHEN THEN ELSE END NOW() LOWER() UPPER()
INTEGER TEXT VARCHAR(255) BOOLEAN DATE TIMESTAMP REAL BLOB DEFAULT UNIQUE CHECK
BEGIN COMMIT ROLLBACK TRANSACTION WITH RECURSIVE OVER PARTITION ROW_NUMBER() RANK()
* = <> != <= >= ; users id name email created_at user_id
""")

lang("bash", "Bash", r"""
#!/usr/bin/env bash set -euo pipefail if then elif else fi for in do done while until case esac function return
echo printf read local export source exit shift test [ ] [[ ]] (( )) $1 $@ $# $? $$ "$@" ${var} $(cmd)
cd ls pwd mkdir rm cp mv cat grep sed awk find xargs sort uniq head tail wc cut tr tee chmod chown
curl wget tar ssh scp git sudo kill ps top df du which env true false /dev/null 2>&1 > >> < | || && ;
-eq -ne -lt -gt -le -ge -z -n -f -d -e ~/ ./ ../ *.txt $HOME $PATH $PWD IFS trap
""")

lang("lua", "Lua", r"""
local function return end if then elseif else for in do while repeat until break goto
and or not nil true false self require
print() pairs() ipairs() type() tostring() tonumber() pcall() error() assert() select() next() setmetatable()
string.format() string.sub() string.find() string.gsub() table.insert() table.remove() table.concat() table.sort()
math.floor() math.max() math.random() os.time() io.write() io.read()
.. ~= == <= >= # {} [] : ... __index __newindex __call M.new() love.draw() vim.api
""")

lang("r", "R", r"""
function return if else for while repeat break next in TRUE FALSE NULL NA NaN Inf library require
<- -> <<- %>% |> %in% == != <= >= & | ! $ @ :: [[ ]]
c() list() vector() matrix() data.frame() factor() seq() rep() length() names() nrow() ncol()
mean() median() sd() sum() max() min() round() paste() paste0() print() cat() str() summary() head()
apply() lapply() sapply() mapply() tapply() is.na() which() order() sort() unique() table()
read.csv() write.csv() ggplot() aes() geom_point() dplyr filter() mutate() select() group_by() summarise()
lm() glm() plot() hist() set.seed() rnorm() runif() df$x
""")

lang("haskell", "Haskell", r"""
module where import qualified as hiding data type newtype class instance deriving
let in case of if then else do return where -> <- => :: = | \ _
Int Integer Double Float Bool Char String Maybe Just Nothing Either Left Right IO () True False
map filter foldr foldl zip zipWith head tail length reverse concat concatMap elem sum product
show read print putStrLn getLine mapM_ forM_ pure fmap <$> <*> >>= >> . $ ++ !! `div` `mod`
Functor Applicative Monad Show Eq Ord Num Monoid Semigroup mempty <> Data.Map Data.List
main :: IO () main = do xs x:xs [] [x] (x, y) fst snd id const flip undefined error
""")

lang("scala", "Scala", r"""
def val var lazy object class trait case sealed abstract final override private protected implicit given using
extends with new this super import package if else match case for yield while do return throw try catch finally
Int Long Double Boolean String Unit Any Nothing Option Some None List Seq Vector Map Set Future Either
map flatMap filter foldLeft foreach collect getOrElse mkString toList size head tail isEmpty
println() => <- -> :: ++ _ ??? s"$x" @tailrec sbt Akka Spark implicitly enum then
""")

lang("dart", "Dart", r"""
void main() return if else for in while do switch case default: break; continue; try catch finally throw
class abstract extends implements with mixin enum extension import export library part
final const var late required static factory get set async await yield sync* async*
int double num String bool List<int> Map<String, Set dynamic Object Future<void> Stream null true false
print() length isEmpty add() addAll() remove() contains() map() where() toList() forEach() toString()
this super new ?? ?. ! => ... '$x' '${x}' @override Widget build(BuildContext context) setState()
StatelessWidget StatefulWidget Text() Column() Row() Container() flutter pubspec.yaml
""")

lang("perl", "Perl", r"""
#!/usr/bin/perl use strict; use warnings; my our local sub return if elsif else unless while until for foreach last next redo
print say printf chomp split join push pop shift unshift keys values each exists delete defined
open close die warn eval ref bless scalar wantarray length substr index lc uc sprintf sort map grep
$x @list %hash $_ @_ $0 $! =~ !~ s/// tr/// qw() => -> :: . .= eq ne lt gt cmp <=> <STDIN> __END__
""")

lang("elixir", "Elixir", r"""
defmodule def defp do end fn -> case cond with if else unless when receive after try rescue catch raise
import alias require use quote unquote true false nil :ok :error @moduledoc @doc @spec @impl
|> <- => ++ <> =~ & &1 %{} [] {} ~r// ~s"" "#{x}" _ ^pin
Enum.map Enum.filter Enum.reduce Enum.each Enum.count Map.get Map.put Map.merge List.first String.split
IO.puts IO.inspect Kernel spawn send self() GenServer Agent Task Supervisor Ecto Phoenix mix iex
""")

lang("zig", "Zig", r"""
const var fn pub return if else while for switch break continue defer errdefer try catch orelse unreachable
struct enum union error opaque comptime inline export extern test and or null undefined true false
u8 u32 u64 i32 i64 usize f32 f64 bool void anyerror anytype type noreturn []const u8 ?*T !void
@import("std") std.debug.print() std.mem.Allocator std.ArrayList std.heap.page_allocator
allocator.alloc() allocator.free() @intCast() @as() @sizeOf() @TypeOf() .{} => |x| ... zig build
""")

out = sys.argv[1]
import os
for name,(display,words) in L.items():
    def ok(w):
        # Whole phrases were split on spaces; drop the fragments.
        pairs = ["()", "[]", "{}"]
        if any(w.count(o) != w.count(c) for o, c in pairs):
            return False
        return not w.endswith(",")
    seen=[]
    dropped=[]
    for w in words:
        if not ok(w):
            dropped.append(w); continue
        if w not in seen: seen.append(w)
    if dropped: print("   dropped", name, dropped)
    lines=[]; cur="  "
    for w in seen:
        tok=json.dumps(w, ensure_ascii=False)+", "
        if len(cur)+len(tok)>88:
            lines.append(cur.rstrip()); cur="  "
        cur+=tok
    lines.append(cur.rstrip().rstrip(","))
    slug=f"code_{name}"
    text=f'name = "{slug}"\ndisplay = "{display} (code)"\nwords = [\n'+"\n".join(lines)+"\n]\n"
    open(os.path.join(out,f"{slug}.toml"),"w").write(text)
    print(f"{slug}: {len(seen)}")
