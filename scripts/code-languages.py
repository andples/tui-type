#!/usr/bin/env python3
"""Writes the programming-language word lists (catalog/languages/code_*.toml):
keywords, operators, built-ins, standard-library names and common idioms per
language, plus optional modules: extra lists for common libraries, written
to catalog/languages/code_<language>/<module>.toml (see `module` below and
docs/language-modules.md). Each list is split on whitespace, so every entry
must be one self-contained token (no spaces inside). Edit the lists below,
run `scripts/code-languages.py catalog/languages`, then regenerate the index
with `TTYP_BLESS=1 cargo test catalog_index`."""
import json, sys
L = {}
M = {}
def lang(name, display, text, trim=()):
    """`trim`: boilerplate the `trim_syntax` setting cuts from every word of
    this language and its modules, e.g. ["()"] for Python calls."""
    L[name] = (display, text.split(), list(trim))
def module(language, name, display, text):
    """An add-on list for `language`, picked in the app's modules checklist.
    Keep library calls here rather than in the base list, so the base stays
    the language itself."""
    M.setdefault(language, []).append((name, display, text.split()))

lang("python", "Python", r"""
def return if elif else for while in not and or is None True False import from as class self try
except finally raise with yield lambda pass break continue global nonlocal assert del async
await match case print() len() range() enumerate() zip() map() filter() sorted() reversed()
sum() min() max() abs() round() int() float() str() bool() list() dict() set() tuple() type()
isinstance() issubclass() hasattr() getattr() setattr() open() input() super() iter() next()
any() all() repr() vars() id() hash() callable() format() ord() chr() divmod() pow() __init__
__name__ __main__ __repr__ __str__ __eq__ __len__ __iter__ __next__ __enter__ __exit__
__getitem__ __call__ __file__ __all__ self.name self.value self.items cls @property
@staticmethod @classmethod @dataclass @abstractmethod append() extend() insert() pop() remove()
index() count() sort() clear() keys() values() items() get() update() copy() setdefault() add()
discard() split() join() strip() rstrip() lstrip() replace() startswith() endswith() lower()
upper() find() encode() decode() read() write() close() readlines() List Dict Set Tuple Optional
Any Union Callable Iterable Iterator TypeVar Literal list[int] list[str] dict[str,int]
Optional[str] Exception ValueError TypeError KeyError IndexError AttributeError RuntimeError
StopIteration NotImplementedError FileNotFoundError ZeroDivisionError == != <= >= += -= *= // **
% -> := ... *args **kwargs args kwargs f"{x}" f"{name}" "__main__" [] {} () [0] [-1] [1:] [:-1]
[::-1] [i] range(n) range(len(xs)) print(f"{x}") len(xs) x y i n xs key value result data item
name path line text
""", trim=["()"])

module("python", "stdlib", "Standard library", r"""
os sys json re math time random itertools functools collections pathlib subprocess logging
typing datetime asyncio dataclasses argparse shutil csv os.path.join() os.path.exists()
os.path.dirname() os.path.basename() os.environ os.getcwd() os.listdir() os.makedirs()
os.remove() sys.argv sys.exit() sys.stdin sys.stdout sys.path json.loads() json.dumps()
json.load() json.dump() re.match() re.search() re.sub() re.compile() re.findall() math.sqrt()
math.floor() math.ceil() math.pi math.inf time.time() time.sleep() time.perf_counter()
random.randint() random.choice() random.shuffle() random.random() random.seed() Path()
Path.cwd() path.read_text() path.write_text() path.exists() path.parent path.name path.suffix
path.glob() datetime.now() datetime.date() timedelta() defaultdict() Counter() namedtuple()
deque() OrderedDict() itertools.chain() itertools.product() itertools.groupby()
functools.partial() functools.reduce() @functools.lru_cache @functools.wraps @functools.cache
subprocess.run() subprocess.check_output() logging.getLogger() logging.basicConfig()
logger.info() logger.debug() logger.warning() logger.error() argparse.ArgumentParser()
parser.add_argument() parser.parse_args() asyncio.run() asyncio.gather() asyncio.sleep()
asyncio.create_task() shutil.copy() shutil.rmtree() csv.reader() csv.writer() csv.DictReader()
field() asdict() __future__ annotations
""")

module("python", "numpy", "NumPy", r"""
numpy np np.array() np.zeros() np.ones() np.empty() np.arange() np.linspace() np.eye() np.full()
np.random.rand() np.random.randn() np.random.randint() np.random.seed() np.random.default_rng()
rng.normal() rng.integers() np.reshape() arr.reshape() arr.shape arr.ndim arr.size arr.dtype
arr.T arr.astype() arr.flatten() arr.ravel() arr.copy() arr.tolist() np.sum() np.mean()
np.median() np.std() np.var() np.min() np.max() np.argmin() np.argmax() np.cumsum() np.prod()
np.abs() np.sqrt() np.exp() np.log() np.sin() np.cos() np.pi np.inf np.nan np.dot() np.matmul()
a@b np.linalg.inv() np.linalg.norm() np.linalg.eig() np.linalg.solve() np.transpose()
np.concatenate() np.stack() np.vstack() np.hstack() np.split() np.where() np.unique() np.sort()
np.argsort() np.clip() np.isnan() np.allclose() np.all() np.any() np.float32 np.float64 np.int32
np.int64 np.bool_ np.ndarray axis=0 axis=1 keepdims=True dtype=np.float32 arr[:,0] arr[0,:]
arr[mask] arr[...,None] np.newaxis np.save() np.load() np.loadtxt() np.savetxt() np.meshgrid()
np.histogram() np.percentile() np.diff() np.round()
""")

module("python", "pandas", "pandas", r"""
pandas pd pd.DataFrame() pd.Series() pd.read_csv() pd.read_excel() pd.read_json()
pd.read_parquet() pd.read_sql() pd.concat() pd.merge() pd.to_datetime() pd.to_numeric()
pd.isna() pd.notna() pd.date_range() pd.cut() pd.get_dummies() pd.pivot_table() pd.NA
pd.Timestamp() df df.head() df.tail() df.info() df.describe() df.shape df.columns df.index
df.dtypes df.values df.to_numpy() df.loc[] df.iloc[] df.at[] df["col"] df[["a","b"]] df.query()
df.filter() df.groupby() df.agg() df.apply() df.map() df.sort_values() df.sort_index()
df.reset_index() df.set_index() df.rename() df.drop() df.dropna() df.fillna() df.astype()
df.assign() df.copy() df.merge() df.join() df.pivot() df.melt() df.explode() df.duplicated()
df.drop_duplicates() df.value_counts() df.nunique() df.sum() df.mean() df.count() df.corr()
df.rolling() df.resample() df.shift() df.diff() df.isna() df.sample() df.to_csv() df.to_json()
df.to_parquet() df.to_dict() df.plot() s.str.contains() s.str.lower() s.str.split() s.dt.year
s.dt.month s.unique() s.tolist() s.idxmax() inplace=True axis=1 ignore_index=True how="left"
on="id" as_index=False index=False
""")

module("python", "pytorch", "PyTorch", r"""
torch torch.nn nn F torch.tensor() torch.zeros() torch.ones() torch.randn() torch.rand()
torch.arange() torch.empty() torch.eye() torch.cat() torch.stack() torch.matmul()
torch.no_grad() torch.manual_seed() torch.save() torch.load() torch.device()
torch.cuda.is_available() torch.float32 torch.long torch.optim torch.utils.data nn.Module
nn.Linear() nn.Conv2d() nn.ReLU() nn.GELU() nn.Sequential() nn.Dropout() nn.BatchNorm2d()
nn.LayerNorm() nn.Embedding() nn.LSTM() nn.MultiheadAttention() nn.CrossEntropyLoss()
nn.MSELoss() nn.Parameter() nn.ModuleList() F.relu() F.softmax() F.cross_entropy() F.mse_loss()
F.dropout() F.log_softmax() x.to(device) x.cuda() x.cpu() x.numpy() x.item() x.view()
x.reshape() x.permute() x.transpose() x.unsqueeze() x.squeeze() x.detach() x.shape x.size()
x.dim() x.float() x.argmax() x.mean() x.sum() x.grad requires_grad=True model model.train()
model.eval() model.parameters() model.state_dict() model.load_state_dict() model.to(device)
self.fc self.conv forward() super().__init__() optimizer optim.Adam() optim.SGD() optim.AdamW()
optimizer.zero_grad() optimizer.step() loss loss.backward() loss.item() scheduler.step()
DataLoader() Dataset TensorDataset() batch_size=32 shuffle=True num_workers=4 loader epoch
logits targets lr=1e-3
""")

module("python", "tensorflow", "TensorFlow", r"""
tensorflow tf keras tf.keras tf.constant() tf.Variable() tf.zeros() tf.ones() tf.random.normal()
tf.random.uniform() tf.range() tf.reshape() tf.cast() tf.concat() tf.stack() tf.matmul()
tf.reduce_sum() tf.reduce_mean() tf.reduce_max() tf.argmax() tf.nn.relu() tf.nn.softmax()
tf.float32 tf.int32 tf.GradientTape() tape.gradient() tf.function @tf.function tf.data.Dataset
tf.data.Dataset.from_tensor_slices() dataset.batch() dataset.shuffle() dataset.map()
dataset.prefetch() tf.data.AUTOTUNE tf.convert_to_tensor() tf.shape() tf.expand_dims()
tf.squeeze() tf.one_hot() tf.where() tf.config.list_physical_devices() tf.saved_model.save()
keras.Sequential() keras.Model() keras.Input() keras.layers layers.Dense() layers.Conv2D()
layers.MaxPooling2D() layers.Flatten() layers.Dropout() layers.BatchNormalization()
layers.Embedding() layers.LSTM() layers.Input() activation="relu" activation="softmax"
model.compile() model.fit() model.evaluate() model.predict() model.summary() model.save()
keras.models.load_model() model.layers model.trainable_variables optimizer="adam"
keras.optimizers.Adam() keras.losses.SparseCategoricalCrossentropy() loss="mse"
metrics=["accuracy"] epochs=10 batch_size=32 validation_split=0.2 callbacks=[]
keras.callbacks.EarlyStopping() keras.callbacks.ModelCheckpoint() from_logits=True
optimizer.apply_gradients() history.history
""")


lang("cpp", "C++", r"""
#include #define #ifdef #ifndef #endif #pragma once
<iostream> <vector> <string> <map> <set> <memory> <algorithm> <unordered_map> <unordered_set>
<utility> <functional> <optional> <array> <cstdint> <cstdio> <thread> <mutex> <chrono> <sstream>
<fstream> <cassert> <cmath>
int main() return return 0; argc argv
namespace using class struct union public: private: protected: virtual override final const constexpr
consteval static inline template typename auto void bool char int long short double float unsigned
signed size_t nullptr true false new delete this operator friend explicit mutable volatile noexcept
enum typedef sizeof decltype alignof static_assert thread_local extern concept requires
co_await co_return co_yield if else for while do switch case default: break; continue; try catch
throw goto
std::cout std::cin std::cerr std::endl std::string std::vector std::map std::set std::unordered_map
std::unordered_set std::unique_ptr std::shared_ptr std::weak_ptr std::make_unique std::make_shared
std::move std::forward std::swap std::pair std::make_pair std::tuple std::optional std::nullopt
std::variant std::array std::thread std::mutex std::lock_guard std::function std::string_view
std::sort std::find std::find_if std::remove_if std::count std::reverse std::fill std::transform
std::for_each std::accumulate std::begin std::end std::min std::max std::abs std::size
std::to_string std::stoi std::getline std::runtime_error std::exception std::ostream std::istream
std::chrono std::size_t std::vector<int> std::vector<std::string> std::unique_ptr<T> std::optional<T>
uint8_t uint32_t uint64_t int32_t int64_t
push_back() emplace_back() pop_back() size() empty() begin() end() cbegin() rbegin() clear()
insert() erase() find() at() front() back() reserve() resize() data() c_str() substr() length()
count() emplace() get() reset() lock() join() first second
const& && auto& auto&& const auto* -> :: << >> ++i i++ != == <= >= += &x *ptr ptr-> this->
~Foo() Foo::Foo() operator== operator<< operator() T&& int& int* char* void* std::string&
[&] [=] [this] {} <T>
static_cast<int> static_cast<size_t> dynamic_cast<T*> reinterpret_cast<char*> const_cast<T&>
= default; delete;
""")

lang("c", "C", r"""
#include #define #undef #ifndef #ifdef #if #elif #else #endif #pragma
<stdio.h> <stdlib.h> <string.h> <stdint.h> <stdbool.h> <assert.h> <errno.h> <math.h> <ctype.h>
<unistd.h> <limits.h> <stddef.h> <time.h> <signal.h> <pthread.h>
int main(void) return return 0; argc argv char** **argv
printf() scanf() fprintf() snprintf() sprintf() puts() putchar() getchar() perror()
stderr stdout stdin
malloc() calloc() realloc() free() memcpy() memset() memmove() memcmp()
strlen() strcpy() strncpy() strcmp() strncmp() strcat() strchr() strstr() strdup() strtol()
atoi() atof() qsort() exit() abort() assert() isdigit() isalpha() isspace() toupper() tolower()
sizeof sizeof(int) sizeof(*p)
void char short int long float double unsigned signed const static extern volatile register auto
struct union enum typedef inline restrict _Bool
if else for while do switch case default: break; continue; goto
size_t ssize_t ptrdiff_t uint8_t uint16_t uint32_t uint64_t int8_t int32_t int64_t uintptr_t
bool true false FILE NULL EOF EXIT_SUCCESS EXIT_FAILURE INT_MAX errno
fopen() fclose() fgets() fputs() fread() fwrite() fseek() ftell() fflush() fgetc() fputc() feof()
open() close() read() write() getenv() time() rand() srand() sleep()
pthread_create() pthread_join() pthread_mutex_lock() pthread_mutex_unlock() pthread_t
__FILE__ __LINE__ __func__
*ptr &x ptr->next p->data node->next -> ++ -- += -= != == <= >= && || << >> ! ~ ^ % &
%d %s %c %f %zu %p %x %ld \n \0 '\0' "%d\n" "%s\n" i++ ++i
(void) (char*) (int) char* void* int* buf[256] buf[i] arr[0] {0} &buf *argv[]
""")

lang("javascript", "JavaScript", r"""
function return const let var if else for while do switch case default: break continue
try catch finally throw new this class extends super constructor static get set async await yield
import export default from as typeof instanceof in of delete void null undefined true false NaN
Infinity debugger
console.log() console.error() console.warn() console.table() JSON.parse() JSON.stringify()
Object.keys() Object.values() Object.entries() Object.assign() Object.freeze() Object.fromEntries()
Array.isArray() Array.from() Array.of() Promise.all() Promise.resolve() Promise.reject()
Promise.race() Promise.allSettled() Promise setTimeout() setInterval() clearTimeout() clearInterval()
requestAnimationFrame() structuredClone() queueMicrotask() fetch()
Math.max() Math.min() Math.floor() Math.ceil() Math.round() Math.random() Math.abs() Math.PI
Date.now() Date parseInt() parseFloat() isNaN() encodeURIComponent() Number.isInteger()
String() Number() Boolean() Symbol() BigInt Map Set WeakMap WeakSet Symbol Error TypeError
RangeError SyntaxError RegExp Proxy Reflect globalThis
map() filter() reduce() forEach() find() findIndex() some() every() includes() indexOf()
lastIndexOf() push() pop() shift() unshift() slice() splice() concat() join() split() trim()
toString() toUpperCase() toLowerCase() startsWith() endsWith() replace() replaceAll() padStart()
repeat() charAt() at() flat() flatMap() sort() reverse() fill() keys() values() entries() has()
get() set() add() delete() then() catch() finally() json() text() bind() call() apply()
hasOwnProperty() length size prototype
document window document.querySelector() document.querySelectorAll() document.getElementById()
document.createElement() addEventListener() removeEventListener() appendChild()
classList.add() classList.toggle() textContent innerHTML event e.preventDefault() e.target.value
localStorage.getItem() localStorage.setItem() alert() location.href
require() module.exports exports process.env process.argv process.exit() __dirname
fs.readFileSync() path.join() npm package.json node_modules
req res err req.body req.params res.json() res.send() res.status(404) app.get() app.listen()
=> === !== == != ?? ?. ... && || ! += ++ -- ** ??= ||= () {} [] ...args `${x}` `${name}`
()
""")

lang("typescript", "TypeScript", r"""
interface type enum namespace declare readonly abstract implements private public protected override
string number boolean any unknown never void null undefined object bigint symbol
function return const let var if else for while switch case default: break continue try catch
finally throw class extends super constructor new this static async await import export default
from as of in typeof instanceof keyof infer satisfies is asserts unique get set yield
Partial<T> Required<T> Readonly<T> NonNullable<T> Awaited<T> Partial<User> Record Pick Omit
Exclude Extract ReturnType Parameters InstanceType
Promise<void> Promise<string> Promise<T> Array<string> Array<T> Set<number> Map
string[] number[] T[] unknown[] <T> Error
?: ! ?. ?? => === !== | & ... && || `${x}`
console.log() console.error() JSON.parse() JSON.stringify() Object.keys() Object.entries()
Object.values() Array.isArray() Array.from() Promise.all() Promise.resolve() setTimeout() fetch()
Math.floor() Math.max() Date.now()
map() filter() reduce() forEach() find() some() every() includes() push() slice() split() join()
trim() then() length toString()
React React.FC JSX.Element React.ReactNode props children useState() useEffect() useMemo()
useCallback() useRef() useState<string>()
@Component() @Injectable() @Input() @Output() @NgModule()
tsconfig.json strict noImplicitAny strictNullChecks esModuleInterop .d.ts index.ts tsc
@ts-ignore @ts-expect-error
""")

lang("ocaml", "OCaml", r"""
let in rec and fun function match with if then else begin end type of module struct sig open
include functor val mutable ref when as try raise exception assert lazy method object class
inherit new private virtual external constraint for to downto while do done nonrec
land lor lxor lsl lsr asr mod
true false () unit int float string bool char list array option exn bytes int64 'a 'b
Some None Ok Error Not_found Invalid_argument Failure Exit
List.map List.iter List.iteri List.mapi List.fold_left List.fold_right List.filter List.filter_map
List.length List.rev List.nth List.hd List.tl List.mem List.assoc List.find List.find_opt
List.exists List.for_all List.sort List.sort_uniq List.concat List.init List.combine List.split
List.append List.flatten
Array.make Array.length Array.get Array.set Array.init Array.map Array.iter Array.of_list
Array.to_list
String.length String.concat String.sub String.get String.split_on_char String.trim
String.uppercase_ascii String.equal String.make
Printf.printf Printf.sprintf Printf.eprintf Format.printf Format.fprintf
print_endline print_string print_int print_newline read_line string_of_int int_of_string
float_of_int int_of_float string_of_float
Hashtbl.create Hashtbl.add Hashtbl.replace Hashtbl.find Hashtbl.find_opt Hashtbl.mem Hashtbl.iter
Hashtbl.remove Option.map Option.value Option.get Option.is_some Result.map Result.bind
Map.Make Set.Make Buffer.create Buffer.add_string Buffer.contents Seq Fun.id Stdlib
compare failwith invalid_arg ignore fst snd not exit incr decr min max abs succ pred
:: @ ^ ; ;; <> = == != < > <= >= + - * / +. *. /. -. |> @@ := ! -> | _ [] [||] ~f ~init
x::xs acc xs let* let+ >>= >|= [@@inline]
Lwt Lwt.return Core ppx_deriving dune opam utop .ml .mli
""")

lang("java", "Java", r"""
public private protected static final abstract class interface extends implements enum record
sealed permits void int long double float boolean char byte short
return if else for while do switch case default: default break; continue; try catch finally throw
throws new this super null true false instanceof import package synchronized volatile transient
native var assert yield
String Object Integer Long Double Boolean Character StringBuilder List ArrayList LinkedList Map
HashMap TreeMap Set HashSet Queue Deque Iterator Iterable Optional
Stream IntStream Collectors Arrays Collections Objects Math System Thread Runnable
CompletableFuture ExecutorService Executors LocalDate LocalDateTime Path Paths
Files Scanner BufferedReader InputStream OutputStream File UUID Random Comparator Comparable
List<String> List<Integer> ArrayList<> HashMap<> Set<Integer> Optional<T> Optional<String>
Stream<T> <T> String[] String... args main
System.out.println() System.out.printf() System.err.println() System.currentTimeMillis()
System.exit() String.format() String.valueOf() String.join() Integer.parseInt() Integer.valueOf()
Integer.MAX_VALUE Math.max() Math.min() Math.abs() Math.random() Arrays.asList() Arrays.sort()
Arrays.stream() Collections.sort() Collections.emptyList() List.of() Map.of() Set.of()
Objects.equals() Objects.requireNonNull() Optional.of() Optional.empty() Optional.ofNullable()
Thread.sleep() Files.readAllLines() Paths.get() Path.of()
@Override @Deprecated @FunctionalInterface @SuppressWarnings("unchecked") @Test
@BeforeEach @Autowired @Service @RestController @GetMapping @Entity
add() get() put() size() isEmpty() contains() containsKey() remove() equals() hashCode()
toString() length() charAt() substring() indexOf() trim() split() toUpperCase() toLowerCase()
append() getOrDefault() putIfAbsent() keySet() values() entrySet() getKey() getValue()
stream() map() filter() collect() Collectors.toList() Collectors.toMap() Collectors.groupingBy()
Collectors.joining() forEach() reduce() sorted() findFirst() anyMatch() toList() orElse()
orElseThrow() ifPresent() isPresent() iterator() hasNext() next() compareTo() close() start()
join() run() getClass() getName() setName() build() builder()
Exception RuntimeException IllegalArgumentException IllegalStateException NullPointerException
IOException IndexOutOfBoundsException UnsupportedOperationException InterruptedException
-> :: != == && || ++ -- += this.name this.value i++ // /** */ @param @return
""")

lang("csharp", "C#", r"""
using namespace class struct interface enum record public private protected internal static
readonly const void int long double float decimal bool char string object var dynamic byte short
uint ulong return if else for foreach in while do switch case default: break; continue; try catch
finally throw new this base null true false is as typeof sizeof nameof async await yield
get; set; init; virtual override abstract sealed partial params ref out required when where with
lock event delegate operator implicit explicit goto not
and or
String Int32 Object Exception List<T> List<int> List<string> Dictionary HashSet<T> IEnumerable<T>
IList<T> IReadOnlyList<T> Task Task<T> Task<int> Action Action<T> Func<T> Span<T>
IDisposable CancellationToken ILogger<T> StringBuilder
DateTime TimeSpan Guid Nullable Tuple KeyValuePair Stream File Path Regex Encoding.UTF8
Main string[] args Program
Console.WriteLine() Console.Write() Console.ReadLine() string.Format() string.Join()
string.IsNullOrEmpty() string.IsNullOrWhiteSpace() string.Empty Math.Max() Math.Min() Math.Abs()
Math.Round() DateTime.Now DateTime.UtcNow Guid.NewGuid() int.Parse() int.TryParse()
Convert.ToInt32() File.ReadAllText() File.WriteAllText() Path.Combine() Task.Run() Task.Delay()
Task.WhenAll() Task.FromResult() Task.CompletedTask JsonSerializer.Serialize()
JsonSerializer.Deserialize<T>() Environment.GetEnvironmentVariable() Enum.Parse()
Enumerable.Range() Array.Empty<T>() ArgumentNullException.ThrowIfNull()
WebApplication.CreateBuilder(args); app.MapGet() app.Run();
Add() AddRange() Remove() Contains() ContainsKey() TryGetValue() Clear() Count Count() Length
ToString() Equals() GetHashCode() GetType() ToList() ToArray() ToDictionary() Where() Select()
SelectMany() OrderBy() OrderByDescending() ThenBy() First() FirstOrDefault() Single()
SingleOrDefault() Last() Any() All() Sum() Max() Min() Average() GroupBy() Distinct() Skip()
Take() Aggregate() Split() Trim() Substring() Replace() StartsWith() IndexOf() ToUpper()
ToLower() Dispose() ConfigureAwait(false) Invoke()
=> ?? ??= ?. ! == != && || ++ += $"{x}" $"" @"" #region #endregion #nullable #if ///
[Serializable] [HttpGet] [HttpPost] [ApiController] [Route("api/[controller]")] [Fact] [Test]
[Required] [Obsolete] [JsonPropertyName("id")]
ArgumentException ArgumentNullException InvalidOperationException NotImplementedException
NullReferenceException KeyNotFoundException OperationCanceledException
""")

lang("go", "Go", r"""
package main import func return var const type struct interface map chan go defer select
if else for range switch case default: break continue fallthrough goto
int int8 int16 int32 int64 uint uint8 uint16 uint32 uint64 uintptr float32 float64 complex128
string bool byte rune error any comparable nil true false iota
make() new() len() cap() append() copy() delete() panic() recover() close() print() println()
min() max() clear() main() init()
fmt.Println() fmt.Printf() fmt.Sprintf() fmt.Errorf() fmt.Fprintf() fmt.Sprint() fmt.Print()
errors.New() errors.Is() errors.As() os.Exit() os.Args os.Getenv() os.Open() os.Create()
os.ReadFile() os.WriteFile() os.Stdout os.Stderr
strings.Split() strings.Join() strings.Contains() strings.HasPrefix() strings.HasSuffix()
strings.TrimSpace() strings.Replace() strings.ToLower() strings.ToUpper() strings.Fields()
strings.Builder strconv.Itoa() strconv.Atoi() strconv.ParseInt() strconv.FormatInt()
time.Now() time.Sleep() time.Since() time.Duration time.Second time.Millisecond time.After()
context.Context context.Background() context.WithCancel() context.WithTimeout() context.TODO()
ctx ctx.Done() cancel()
http.HandleFunc() http.ListenAndServe() http.Get() http.NewRequest() http.StatusOK http.Handler
http.ResponseWriter *http.Request w.Header() w.WriteHeader() r.URL r.Body
json.Marshal() json.Unmarshal() json.NewEncoder() json.NewDecoder()
sync.WaitGroup sync.Mutex sync.RWMutex sync.Once wg.Add() wg.Done() wg.Wait() mu.Lock() mu.Unlock()
io.Reader io.Writer io.EOF io.ReadAll() io.Copy() bufio.NewScanner() bufio.NewReader()
scanner.Scan() scanner.Text() log.Println() log.Printf() log.Fatal() sort.Slice() sort.Ints()
slices.Sort() slices.Contains() filepath.Join() bytes.Buffer regexp.MustCompile() math.MaxInt
rand.Intn()
:= <- ... != == && || ++ -- += & * _ ok err &T{} *T
[]string []byte []int map[string]int map[string]any <-chan chan<- struct{} interface{} func()
[]byte(s) string(b)
t.Fatal() t.Errorf() t.Run() *testing.T String() Error() ServeHTTP()
go.mod go.sum gofmt //go:embed //go:generate
""")

lang("rust", "Rust", r"""
fn let mut const static struct enum impl trait pub pub(crate) use mod crate self Self super
match if else loop while for in return break continue move ref as where unsafe async await dyn
type extern macro_rules!
i32 i64 u8 u32 u64 usize f32 f64 bool char str &str String
Vec<T> Vec<u8> Vec<String> Option<T> Result<T> Result<()> Box<T> Rc<T> Arc<T> RefCell<T>
Mutex HashMap HashSet BTreeMap Path PathBuf Duration Instant
Ordering Default Debug Display Clone Copy PartialEq Eq Hash PartialOrd Ord Send Sync
Iterator From Into Fn FnMut FnOnce Error
impl<T> <T> 'a 'static &'static
Some() Ok() Err() None true false Ok(()) ?
Self::new() Default::default() String::new() String::from() Vec::new()
HashMap::new() Box::new() Arc::new()
std::mem::take() fs::read_to_string()
thread::spawn()
unwrap() expect() unwrap_or() unwrap_or_default() unwrap_or_else() map_err() and_then()
ok() is_some() is_none() take() clone() to_string() into()
iter() iter_mut() into_iter() collect() map() filter() filter_map() flat_map() fold() enumerate()
zip() rev() any() all() find() count() sum() max() min()
skip() cloned()
sort() sort_by() extend() len() push() pop() is_empty() contains()
get() insert() remove() entry() or_insert() keys() values()
as_str() as_ref() chars() split() trim()
parse() parse::<i32>() starts_with() push_str() first() last()
borrow() lock() join() new() default() from() next()
println!() print!() eprintln!() format!() write!() vec![] assert!() assert_eq!()
assert_ne!() panic!() todo!() unimplemented!() unreachable!() matches!() dbg!()
#[derive(Debug)] #[derive(Default)] #[test] #[cfg(test)] #[allow(dead_code)] #[inline]
#[tokio::main] #[serde(default)]
&self &mut -> => :: .. ..= |x| |_| || && &x *x _ += == !=
std::io std::fs std::fmt std::collections std::sync::Arc std::collections::HashMap;
fmt::Result io::Result<()> anyhow::Result<()> super::*
serde tokio anyhow Serialize Deserialize cargo Cargo.toml
""")

lang("php", "PHP", r"""
<?php ?> echo print function return if else elseif foreach as while for do switch case default:
break; continue; class interface trait extends implements public private protected static final
abstract new namespace use require require_once include include_once global const instanceof
clone yield list() fn match readonly enum
null true false string int float bool mixed void never iterable callable object ?string ?int self
$this $x $i $arr $key $value $data $result $request $_GET $_POST $_SERVER $_SESSION $_COOKIE
$_FILES $this-> $this->id $this->name self:: parent:: static:: parent::__construct()
__construct() __destruct() __toString() __get() __call() __DIR__ __CLASS__ __FUNCTION__ PHP_EOL
array() [] isset() empty() unset() count() strlen() str_replace() str_contains()
str_starts_with() explode() implode() trim() strtolower() strtoupper() substr() strpos()
sprintf() printf() preg_match() preg_replace() htmlspecialchars() number_format() intval()
is_array() is_null() is_string() is_numeric() array_map() array_filter() array_keys()
array_values() array_merge() array_push() array_pop() array_key_exists() array_search()
array_slice() array_reduce() array_unique() in_array() sort() usort() ksort() json_encode()
json_decode() var_dump() print_r() die() exit() file_get_contents() file_put_contents() date()
time() strtotime() microtime() password_hash() password_verify() define() function_exists()
header() session_start() setcookie() http_response_code()
try catch finally throw Exception InvalidArgumentException RuntimeException Throwable TypeError
PDO PDOException $pdo->prepare() $stmt->execute() $stmt->fetch() $stmt->fetchAll()
PDO::FETCH_ASSOC mysqli_query()
=> -> :: . .= === !== == != ?? ??= ?-> <=> && || ! ++ declare(strict_types=1); #[Override]
composer.json Laravel Route::get() $request->input() view() collect()
""")

lang("ruby", "Ruby", r"""
def end class module if elsif else unless case when while until for in do begin rescue ensure
raise return yield self nil true false and or not then next break redo retry super alias undef
defined? __method__ __FILE__ __dir__
require require_relative include extend prepend attr_accessor attr_reader attr_writer private
protected public puts print p pp gets new initialize lambda proc loop catch throw format sleep
exit block_given?
each map select reject reduce each_with_index each_with_object times upto downto step inject
collect detect find find_all group_by partition sort sort_by min_by max_by sum count zip flatten
compact uniq take drop first last push pop shift unshift length size tally filter_map
empty? nil? include? any? all? none? is_a? respond_to? key? frozen? start_with? end_with? zero?
even? odd?
reverse join split strip chomp gsub sub upcase downcase capitalize to_s to_i to_f to_a to_h
to_sym to_proc inspect freeze dup tap then send public_send define_method method_missing fetch
dig merge keys values each_pair transform_values slice
@name @@count :symbol :name => -> ->(x) {} [] |x| |item| |i| &:to_s &block *args **opts &.
||= << <=> == === =~ .. ... "#{x}" "#{name}" %w[] %i[]
Hash.new Array.new Struct.new Class.new Hash Array String Integer Float Symbol Proc Range Set
Time.now File.read File.open File.join Dir.glob JSON.parse ENV ARGV StandardError ArgumentError
RuntimeError NoMethodError NotImplementedError Comparable Enumerable Kernel Object
rails bundle gem Gemfile rake rspec describe it context let before expect eq to be_truthy
has_many belongs_to validates before_action params render redirect_to ActiveRecord::Base
ApplicationController find_by where create save update destroy
""")

lang("swift", "Swift", r"""
func let var return if else guard switch case default: for in while repeat break continue
fallthrough class struct enum protocol extension init deinit self Self super import public private
internal fileprivate open static final lazy weak unowned mutating nonmutating inout throws
rethrows throw try try? try! catch do defer async await actor some any where associatedtype
typealias subscript convenience required override is as as? as! nil true false get set willSet
didSet #if #available #selector @available
Int Int64 UInt Double Float String Bool Character Array Dictionary Set Optional Any AnyObject Void
Never Error Result Data Date URL UUID Codable Decodable Encodable Equatable Hashable Identifiable
Comparable CustomStringConvertible Sendable Task ObservableObject Int? String? [String] [Int]
[Int]()
print() count isEmpty first last description append() insert() remove() removeAll()
removeLast() contains() map() compactMap() flatMap() filter() reduce() forEach() sorted()
sorted(by:) sort() reversed() joined() split() enumerated() zip() min() max() firstIndex(of:)
prefix() dropFirst() lowercased() uppercased() hasPrefix() hasSuffix() String(describing:)
String() Int() abs() stride(from:to:by:) fatalError() precondition() assert()
DispatchQueue.main.async Task.sleep() JSONDecoder() JSONEncoder() URLSession.shared
NotificationCenter.default UserDefaults.standard FileManager.default Date() UUID()
?? ?. ! -> ... ..< == != === && || += "\(x)" "\(name)" $0 $1 _
@State @Binding @Published @MainActor @escaping @objc @ObservedObject @StateObject
@EnvironmentObject @Environment @discardableResult @main @Observable @ViewBuilder @IBOutlet
@IBAction @testable
SwiftUI Foundation UIKit XCTest View body: Text() VStack HStack ZStack Button() Image() List
NavigationStack ForEach Spacer() .padding() .frame() .onAppear() XCTAssertEqual()
UIViewController viewDidLoad()
""")

lang("kotlin", "Kotlin", r"""
fun val var return if else when for in while do break continue class object interface enum data
sealed open abstract override private public internal protected companion init constructor this
super import package null true false is !is as as? !in typealias suspend inline reified lateinit
by lazy const operator infix vararg tailrec noinline crossinline value annotation inner out where
throw try catch finally
Int Long Short Byte Double Float Boolean String Char Unit Any Nothing Array List MutableList Map
MutableMap Set MutableSet Pair Triple Sequence IntArray StringBuilder Result Regex Exception
IllegalArgumentException IllegalStateException Int? String? List<String> List<Int>
MutableList<T> Array<String> Flow<T> StateFlow MutableStateFlow Job Deferred CoroutineScope
Dispatchers.IO Dispatchers.Main
listOf() mutableListOf() mapOf() mutableMapOf() setOf() mutableSetOf() emptyList() arrayOf()
intArrayOf() hashMapOf() println() print() require() requireNotNull() check() checkNotNull()
error() TODO() repeat() also apply let run with takeIf use main() args
map filter forEach first last size isEmpty() isNotEmpty() isNullOrEmpty() isBlank() contains()
joinToString() toString() toInt() toList() toMutableList() toSet() toMap() mapNotNull()
flatMap() filterNot() groupBy associateBy sortedBy() sortedByDescending() sumOf() count() any()
all() none() find() firstOrNull() getOrNull() getOrElse() getOrPut() zip() withIndex()
forEachIndexed() indices lastIndex reversed() distinct() chunked() windowed() take() drop()
split() trim() substring() startsWith() replace() uppercase() lowercase() format() copy()
equals() hashCode() invoke()
it it.name launch async await() delay() runBlocking withContext() coroutineScope
viewModelScope flow emit() collect()
?. ?: !! -> :: .. ..< until downTo step $name "${x}" "$name" ::class ::class.java == === != &&
@JvmStatic @JvmOverloads @JvmField @Composable @Test @Suppress("UNUSED") @Serializable @Inject
@Deprecated @Volatile
Modifier remember mutableStateOf() setContent Column Row Text()
""")

lang("sql", "SQL", r"""
SELECT FROM WHERE AND OR NOT IN IS NULL LIKE ILIKE BETWEEN EXISTS DISTINCT AS ON USING
INSERT INTO VALUES UPDATE SET DELETE CREATE TABLE ALTER ADD COLUMN DROP INDEX VIEW PRIMARY KEY
FOREIGN REFERENCES CASCADE CONSTRAINT RENAME TO TRUNCATE IF
JOIN LEFT RIGHT INNER OUTER FULL CROSS NATURAL GROUP BY ORDER HAVING LIMIT OFFSET FETCH FIRST ROWS
ONLY UNION ALL INTERSECT EXCEPT ASC DESC NULLS LAST CASE WHEN THEN ELSE END
COUNT(*) COUNT() SUM() AVG() MIN() MAX() COALESCE() NULLIF() CAST() EXTRACT() NOW() CURRENT_DATE
CURRENT_TIMESTAMP DATE_TRUNC() LOWER() UPPER() TRIM() LENGTH() SUBSTRING() CONCAT() REPLACE()
ROUND() ABS() IFNULL() STRING_AGG() GROUP_CONCAT() ARRAY_AGG() ROW_NUMBER() RANK() DENSE_RANK()
LAG() LEAD() OVER PARTITION WINDOW
INTEGER INT BIGINT SMALLINT SERIAL TEXT VARCHAR VARCHAR(255) CHAR(10) BOOLEAN DATE TIME TIMESTAMP
TIMESTAMPTZ INTERVAL REAL FLOAT DOUBLE NUMERIC DECIMAL(10,2) BLOB JSON JSONB UUID
DEFAULT UNIQUE CHECK AUTOINCREMENT AUTO_INCREMENT
BEGIN COMMIT ROLLBACK SAVEPOINT TRANSACTION WITH RECURSIVE RETURNING CONFLICT DO NOTHING EXPLAIN
ANALYZE GRANT REVOKE TRIGGER FUNCTION PROCEDURE DATABASE SCHEMA SEQUENCE VACUUM PRAGMA ANY SOME
MERGE MATCHED
* = <> != < > <= >= ; || % . -- ? $1
users orders products id name email status amount price total created_at updated_at user_id
order_id u.id o.user_id users.id 'active'
""")

lang("bash", "Bash", r"""
#!/bin/bash #!/usr/bin/env bash sh set -euo pipefail -e -u -x -o
if then elif else fi for in do done while until case esac function return select break continue ;;
echo printf read local declare readonly export unset source . exit shift test eval exec trap wait
alias type command getopts let shopt mapfile readarray
$1 $2 $0 $@ $# $? $$ $! $* "$@" "$1" ${var} ${1:-} ${#arr[@]} ${arr[@]}
"${arr[@]}" ${var:-default} ${var%.*} ${var##*/} $(pwd) "$(pwd)" $((i+1)) ((i++)) i=0
"${BASH_SOURCE[0]}" $RANDOM $HOME $PATH $PWD $USER $SHELL $LINENO IFS IFS=$'\n' OPTARG
$i $f $file "$file" "$var"
cd ls pwd mkdir rm cp mv cat grep sed awk find xargs sort uniq head tail wc cut tr tee chmod chown
ln touch basename dirname date sleep diff less jq curl wget tar gzip unzip zip ssh scp rsync git
sudo kill pkill ps top df du which whoami env true false yes seq
/dev/null 2>&1 >/dev/null > >> < << <<EOF EOF <<< | || && ; & !
-eq -ne -lt -gt -le -ge -z -n -f -d -e -r -w -x -s == != =~
~/ ./ ../ *.txt *.sh
-rf -p -v -r -i -l -a -c -h -q -E --help -name -type -exec -print0 -maxdepth {} \;
""")

lang("lua", "Lua", r"""
local function return end if then elseif else for in do while repeat until break goto
and or not nil true false self require
print() pairs() ipairs() type() tostring() tonumber() pcall() xpcall() error() assert() select()
next() setmetatable() getmetatable() rawget() rawset() rawequal() rawlen() unpack() load()
loadfile() dofile() collectgarbage()
coroutine.create() coroutine.resume() coroutine.yield() coroutine.wrap() coroutine.status()
string.format() string.sub() string.find() string.gsub() string.gmatch() string.match()
string.len() string.rep() string.byte() string.char() string.lower() string.upper()
string.reverse() s:sub() s:find() s:gsub() s:match() s:gmatch() s:upper() s:lower() s:len()
table.insert() table.remove() table.concat() table.sort() table.pack() table.unpack()
math.floor() math.ceil() math.max() math.min() math.abs() math.sqrt() math.random()
math.randomseed() math.huge math.pi math.fmod() math.tointeger()
os.time() os.clock() os.date() os.getenv() os.exit() os.remove() os.rename()
io.write() io.read() io.open() io.lines() io.stdout f:read() f:write() f:close() f:lines()
utf8.char() utf8.len() package.path package.loaded _G _ENV _VERSION arg
.. ~= == <= >= < > = # {} [] : ... -- #t #list t[#t+1] t[i] t.x k v i
M M.new() self.x obj:method() {...} __index __newindex __call __tostring __eq __lt __le __add
__concat __len __gc __mode __metatable
love.load() love.update(dt) love.draw() love.keypressed() love.graphics.print()
love.graphics.rectangle() dt
vim.api vim.opt vim.g vim.o vim.fn vim.cmd() vim.keymap.set() vim.notify()
vim.api.nvim_create_autocmd() vim.api.nvim_set_keymap() vim.tbl_deep_extend()
""")

lang("r", "R", r"""
function return if else for while repeat break next in TRUE FALSE NULL NA NA_integer_
NA_character_ NaN Inf function(x) \(x)
<- -> <<- %>% |> %in% == != <= >= & | && || ! $ @ :: ~ %% %/% ^ : 1:10 x[i] x[[i]] [[1]] df[1,]
df$x df$y df data x y na.rm stringsAsFactors .data
library() require() library(dplyr) library(ggplot2) library(tidyverse) install.packages() source()
c() list() vector() matrix() array() data.frame() tibble() factor() levels() nlevels() seq()
seq_len() seq_along() rep() length() names() colnames() rownames() nrow() ncol() dim()
mean() median() sd() var() sum() max() min() range() round() abs() sqrt() exp() log() cumsum()
quantile() cor()
paste() paste0() sprintf() format() print() cat() message() warning() stop() str() summary()
head() tail() class() typeof() is.null() is.na() is.numeric() is.character() as.numeric()
as.character() as.integer() as.factor() as.Date() nchar() substr() toupper() tolower() gsub()
sub() grepl() grep() strsplit() trimws()
apply() lapply() sapply() vapply() mapply() tapply() Map() Reduce() Filter() do.call() which()
which.max() order() sort() rev() unique() duplicated() table() any() all() ifelse() switch()
tryCatch() invisible() return() on.exit() stopifnot() identical() match() unlist() setNames()
rbind() cbind() merge() subset() with() split()
Sys.time() Sys.Date() Sys.getenv() file.path() file.exists() readRDS() saveRDS() read.csv()
write.csv() readLines() writeLines() setwd() getwd()
set.seed() rnorm() runif() rbinom() sample() lm() glm() predict() anova() t.test()
plot() hist() lines() points() abline() legend() par() dev.off() png() boxplot() barplot()
ggplot() aes() geom_point() geom_line() geom_bar() geom_histogram() facet_wrap() labs()
theme_minimal() filter() mutate() select() group_by() summarise() summarize() arrange()
left_join() pull() rename() distinct() n() across() everything() pivot_longer() pivot_wider()
read_csv() str_detect() str_replace() map() map_dbl() glimpse()
""")

lang("haskell", "Haskell", r"""
module where import qualified as hiding data type newtype class instance deriving let in case of
if then else do infixl infixr infix forall
-> <- => :: = | \ _ @ ~ ! .. \x \_ otherwise
Int Integer Double Float Bool Char String Maybe Just Nothing Either Left Right IO () True False
Ordering LT GT EQ Word Rational Text ByteString Map Set Vector IORef MVar TVar STM Proxy
[a] [Int] [String] [Char] [] [x] x:xs (x:xs) xs acc go main
map filter foldr foldl foldl' foldMap zip zipWith head tail init last length null
reverse concat concatMap elem lookup sum product maximum minimum and or any all take drop
takeWhile dropWhile span break splitAt replicate iterate repeat lines unlines words unwords
show read print putStrLn putStr getLine readFile writeFile
mapM mapM_ forM forM_ traverse when unless void
pure return fmap
<$> <$ <*> *> <* >>= >> =<< >=> <|> <> . $ $! ++ !! `div` `mod` `elem`
fromIntegral floor ceiling round truncate div mod even odd max
min compare succ pred fst snd id const flip undefined error
maybe either fromMaybe mapMaybe isJust isNothing sortBy sortOn groupBy nub
partition intercalate
Functor Applicative Monad Alternative Show Eq Ord Num Enum Read Integral Fractional
Foldable Traversable Monoid Semigroup
mempty mconcat empty fromList toList Map.insert Map.lookup Map.empty Map.fromList
Map.findWithDefault Data.Map Data.List Data.Maybe Data.Char Data.Text Data.IORef Control.Monad
System.IO Text.Printf printf newIORef readIORef modifyIORef T.pack T.unpack
LANGUAGE OverloadedStrings ScopedTypeVariables stack cabal ghci
""")

lang("scala", "Scala", r"""
def val var lazy object class trait case sealed abstract final override private protected
implicit given using extends with new this super import package if else match for yield while
do return throw try catch finally enum then export extension opaque inline transparent derives
end type
Int Long Double Float Boolean String Char Byte Unit Any AnyRef AnyVal Nothing Null Option Some
None List Nil Seq Vector Array ArrayBuffer ListBuffer Map Set Future Promise Either Left Right Try
Success Failure Iterator LazyList Range Tuple2 StringBuilder BigInt BigDecimal Ordering Throwable
Exception IllegalArgumentException App ExecutionContext
Option[T] Option[Int] List[Int] List[String] Seq[A] Future[Unit] Array[String] [A] [T] [+A] [_]
map flatMap filter filterNot foldLeft foldRight fold reduce foreach collect collectFirst
getOrElse orElse mkString toList toSeq toMap toSet toVector toArray size length head headOption
tail last lastOption isEmpty nonEmpty isDefined contains exists forall find count sum max min
maxBy minBy sortBy sorted sortWith groupBy groupMapReduce partition zip zipWithIndex take drop
takeWhile dropWhile distinct reverse flatten grouped sliding indices updated appended prepended
get apply unapply copy recover andThen compose
println() print() require() assert() Option() Some() List() Map() Seq() Vector() Set() Array()
Future.successful() Await.result()
s"$x" s"${x}" f"$x%.2f" _ => <- -> :: ::: ++ +: :+ == != && || ??? _*
@tailrec @main @volatile @deprecated @inline
implicitly summon classOf sbt build.sbt scala.util scala.collection.mutable Akka Spark ZIO cats IO
""")

lang("dart", "Dart", r"""
void main() return if else for in while do switch case default: break; continue; try catch finally
throw rethrow on class abstract extends implements with mixin enum extension import export library
part show hide deferred final const var late required static factory get set async await yield
sync* async* covariant operator typedef external interface sealed base when assert is is! as this
super new null true false Function
int double num String bool List Map Set Iterable dynamic Object Never Future Stream Duration
DateTime RegExp StringBuffer Uri Exception Error StateError ArgumentError FormatException
List<int> List<String> Set<int> Future<void> Future<String> Stream<int> int? String?
print() length isEmpty isNotEmpty first last keys values entries reversed add() addAll()
remove() removeWhere() removeAt() insert() clear() contains() containsKey() indexOf() map()
where() firstWhere() any() every() fold() reduce() expand() toList() toSet() forEach() join()
split() trim() toUpperCase() toLowerCase() startsWith() substring() replaceAll() toString()
toStringAsFixed() putIfAbsent() sort() compareTo() listen() cast<T>() whereType<T>() then()
catchError() identical() debugPrint()
int.parse() double.parse() int.tryParse() jsonEncode() jsonDecode() DateTime.now() Duration()
Future.delayed() Future.wait() Future.value()
?? ??= ?. ! => ... ...? .. ?.. == != && || '$x' '${x}' "$name" @override @immutable
'package:flutter/material.dart'; 'dart:async'; 'dart:convert'; 'dart:io';
Widget BuildContext context setState() StatelessWidget StatefulWidget State<MyApp> initState()
dispose() super.initState() build() runApp() Text() Column() Row() Container() Padding()
SizedBox() Center() Scaffold() AppBar() ElevatedButton() ListView.builder() MaterialApp()
EdgeInsets.all() Colors.blue Navigator.push() Navigator.pop() Theme.of(context)
MediaQuery.of(context) child: children: onPressed: super.key flutter pubspec.yaml
""")

lang("perl", "Perl", r"""
#!/usr/bin/perl #!/usr/bin/env perl use strict; warnings; my our local sub return if elsif else
unless while until for foreach last next redo do package require no BEGIN END __END__ __DATA__
__PACKAGE__ __FILE__ __LINE__ and or not x eq ne lt gt le ge cmp
print say printf chomp chop split join push pop shift unshift splice keys values each exists
delete defined undef open close die warn eval ref bless scalar wantarray length substr index
rindex lc uc lcfirst ucfirst sprintf sort reverse map grep abs int sqrt rand srand time localtime
sleep exit system exec binmode opendir readdir closedir mkdir unlink rename wait
$x $self $class $_ @_ $0 $1 $2 $! $@ $$ $/ @ARGV %ENV $ENV{HOME} @list %hash $hash{key}
$array[0] @array $#array scalar(@list) @{$ref} %{$ref} $ref->{key} $ref->[0] $self->{name}
\@list \%hash $$ref STDIN <STDIN> STDOUT STDERR <$fh> $fh <>
=~ !~ s/// tr/// m// qw() qw// q() qq() => -> :: . .= == != <=> && || // //= ||= ** ++ -- ..
s/foo/bar/g /^\s+$/ \d+ \w+ /i /g <<EOF EOF -e -f -d
Data::Dumper Dumper() Getopt::Long GetOptions() File::Basename List::Util Scalar::Util blessed()
POSIX Carp croak confess JSON::PP Moose has DBI DBI->connect() $dbh->prepare() $sth->execute()
@ISA SUPER::new() new() parent
""")

lang("elixir", "Elixir", r"""
defmodule def defp defmacro defstruct defimpl defprotocol defdelegate defguard defexception do end
fn -> case cond with if else unless when receive after try rescue catch raise reraise throw for
in import alias require use quote unquote true false nil and or not do: else:
:ok :error :noreply :reply :stop @moduledoc @doc @spec @impl @type @typedoc @behaviour @callback
@derive @enforce_keys @tag
|> <- => ++ -- <> =~ & &1 &2 %{} [] {} :: .. | \\ ^x _ "#{x}" ~r// ~s() ~w() ~D[2024-01-01]
%User{} %__MODULE__{} __MODULE__ [h|t] [head|tail] &Enum.map/2 &IO.puts/1
Enum.map Enum.filter Enum.reduce Enum.each Enum.count Enum.sum Enum.sort Enum.sort_by Enum.find
Enum.any? Enum.all? Enum.member? Enum.into Enum.join Enum.group_by Enum.with_index Enum.zip
Enum.take Enum.uniq Enum.flat_map Enum.reverse Enum.at Enum.max Enum.min Enum.chunk_every
Enum.reject Enum.map_join
Map.get Map.put Map.merge Map.fetch Map.fetch! Map.delete Map.keys Map.values Map.new Map.update
Map.has_key? List.first List.last List.flatten List.wrap
String.split String.trim String.length String.upcase String.downcase String.to_integer
String.contains? String.replace String.starts_with? Integer.to_string Integer.parse Keyword.get
Stream.map Stream.filter IO.puts IO.inspect IO.gets Kernel spawn spawn_link send self()
Process.sleep Process.send_after
GenServer GenServer.start_link GenServer.call GenServer.cast handle_call handle_cast handle_info
init start_link Agent Agent.start_link Agent.get Task Task.async Task.await Supervisor
Supervisor.start_link Registry Application Logger Logger.info Logger.error
File.read! File.write! Path.join Jason.encode! Jason.decode!
is_nil is_binary is_integer is_list is_map elem hd tl length to_string inspect put_elem apply
Ecto Ecto.Changeset Repo.all Repo.get Repo.insert cast validate_required Phoenix conn plug
render json mix mix.exs iex ExUnit.Case test assert refute describe setup doctest
""")

lang("zig", "Zig", r"""
const var fn pub return if else while for switch break continue defer errdefer try catch orelse
unreachable struct enum union error opaque comptime inline noinline export extern test and or
null undefined true false packed align allowzero volatile noalias threadlocal callconv
linksection anyframe suspend resume nosuspend asm
u8 u16 u32 u64 u128 usize i8 i16 i32 i64 isize f16 f32 f64 f128 bool void anyerror anytype
anyopaque type noreturn comptime_int comptime_float c_int
[]u8 []const []T [*]u8 [4]u8 [_]u8 ?*T ?T !void *const *T *Self anyerror!void
@import("std") @import("std"); @import("builtin") @intCast() @as() @sizeOf() @TypeOf() @This()
@field() @ptrCast() @floatFromInt() @intFromFloat() @intFromEnum() @enumFromInt() @truncate()
@min() @max() @memcpy() @memset() @panic() @compileError() @embedFile() @hasDecl() @typeInfo()
@tagName() @errorName() @divTrunc() @mod() @rem() @bitCast() @alignOf() @src() @Vector()
@splat() @abs() @sqrt()
std std.debug.print() std.debug.assert() std.mem.Allocator std.mem.eql() std.mem.indexOf()
std.mem.splitScalar() std.mem.tokenizeScalar() std.mem.sort() std.fmt.allocPrint()
std.fmt.bufPrint() std.fmt.parseInt() std.ArrayList std.StringHashMap std.AutoHashMap
std.heap.page_allocator std.heap.GeneralPurposeAllocator std.heap.ArenaAllocator
std.testing.allocator std.testing.expect() std.testing.expectEqual() std.fs.cwd()
std.process.argsAlloc() std.process.exit() std.log.info() std.log.err() std.math.maxInt()
std.time.milliTimestamp() std.Thread std.meta std.json std.ascii
allocator allocator.alloc() allocator.free() allocator.create() allocator.destroy()
allocator.dupe() list.deinit() list.append() list.items map.put() map.get() gpa.allocator()
arena.deinit() .deinit() .init() .items .len .ptr .{} .? .* x.* &x
=> |x| |*x| |i| |err| ... .. ++ ** +% -% *% << >> == != and or
Self self main() zig build build.zig build.zig.zon
""")

# Symbols are written as the editor abbreviations Lean users type (\R for
# ℝ, \all or \forall for ∀, \r or \to for →; from vscode-lean4's
# abbreviations.json): the symbols themselves aren't on a keyboard.
lang("lean", "Lean 4", r"""
def theorem example abbrev structure inductive instance class namespace section end
variable open where match with fun \fun \la let have show calc by do if then else return mutual
deriving extends private protected noncomputable partial unsafe
universe import at for in mut termination_by decreasing_by
set_option
#eval #check
intro intros apply exact rfl simp simp_all simpa rw rwa induction cases rcases obtain constructor
refine omega decide trivial contradiction exfalso
unfold assumption exists left right ext
funext by_cases by_contra split next case all_goals first
repeat sorry exact? apply? absurd <;> \.
Nat Int Prop Type Sort List Array Option IO String Bool True False Unit
DecidableEq Inhabited Repr
ToString BEq Monad Eq Iff And Or Not Exists
\N \Z \a \b \forall \all \exists \ex \to \r \iff \and \or \not \ne \le \ge \<> <|> <| |> \l := => -> <- <-> /\ \/ \x \in
>>= <$> <*> ++ :: == != && || ! _ ?_
none some zero succ .none .some Nat.succ Nat.zero n+1 x::xs []
#[] ih h h.1 h.2 .mp .mpr Iff.intro And.intro Or.inl Or.inr
Eq.symm congrArg Classical.em Nat.add_comm Nat.zero_add
add_comm zero_add
List.map List.filter List.foldl List.foldr List.length List.reverse
xs.map xs.length arr[i]! arr[i]?
toString s!"{x}" IO.println IO.print pure get set panic!
Option.get! Id.run main
@[simp] @[inline] lake lakefile.lean Lean
""")

# Lean modules. Symbols use the same editor abbreviations as the base list.
module("lean", "mathlib", "Mathlib", r"""
import Mathlib Mathlib.Tactic Mathlib.Data.Real.Basic Mathlib.Data.Nat.Prime.Basic
Mathlib.Data.Finset.Basic Mathlib.Algebra.Group.Basic Mathlib.Analysis.SpecialFunctions.Pow.Real
Mathlib.Topology.Basic Mathlib.Order.Basic lemma Type* \R \C \Q \sum \prod \sub \subseteq \cap
\cup \circ \mapsto \-1 \inf \sup \bot \top \| \nhds linarith nlinarith positivity ring ring_nf
field_simp norm_num push_neg use gcongr polyrith norm_cast push_cast exact_mod_cast zify qify
lift choose filter_upwards continuity fun_prop interval_cases fin_cases tauto aesop bound abel
group nth_rewrite Real Real.sqrt Real.exp Real.log Real.pi Real.sin Real.cos Nat.Prime
Nat.factorial Nat.choose Nat.gcd Finset Finset.range Finset.sum Finset.card Finset.sum_range_succ
Set Set.univ Set.mem_setOf_eq Function.Injective Function.Surjective Continuous Differentiable
deriv Filter.Tendsto Filter.atTop Metric.ball IsOpen IsClosed Polynomial Matrix Group CommGroup
Ring CommRing Field Module Subgroup Ideal MonoidHom LinearMap abs_nonneg sq_nonneg mul_pos
add_pos le_refl le_trans le_antisymm lt_of_le_of_lt lt_irrefl mul_comm mul_assoc pow_two
two_mul sub_nonneg mul_le_mul ne_of_gt Nat.succ_le_iff
""")

module("lean", "std", "Standard library", r"""
import Std Std.Data.HashMap Std.Data.HashSet Std.HashMap Std.HashSet Std.HashMap.empty
m.insert m.get? m.getD m.contains m.erase m.fold m.toList m.size Std.Format IO.FS.readFile
IO.FS.writeFile IO.FS.lines IO.FS.Handle IO.getStdin IO.getStdout stdin.getLine
stdout.putStrLn IO.Process.exit IO.Ref IO.mkRef ref.get ref.set ref.modify IO.monoMsNow
IO.sleep IO.asTask Task.get IO.userError throw tryCatch try catch finally StateT ReaderT ExceptT
StateM modify modifyGet read liftM String.splitOn s.splitOn s.trim s.toNat? s.toNat! s.toList
String.join String.intercalate s.length s.push s.append s.startsWith Char.isDigit c.toNat
Array.range arr.push arr.pop arr.size arr.foldl arr.map arr.toList arr.qsort arr.modify
List.range List.iota l.head! l.tail l.zip List.sum l.take l.drop bv_decide
""")

module("lean", "batteries", "Batteries", r"""
import Batteries Batteries.Data.RBMap Batteries.Data.BinaryHeap Batteries.Data.UnionFind
Batteries.Data.List.Basic Batteries.Data.List.Lemmas Batteries.Data.Array.Lemmas
Batteries.RBMap Batteries.RBSet Batteries.BinaryHeap Batteries.UnionFind RBMap.empty
RBMap.insert RBMap.find? RBMap.contains RBMap.erase RBMap.toList RBMap.foldl compare Ordering
Ordering.lt Ordering.eq Ordering.gt exacts trans alias #help #lint @[nolint]
Batteries.Tactic.Lint Batteries.Data.ByteArray
""")


out = sys.argv[1]
import os
def ok(w):
    # Whole phrases were split on spaces; drop the fragments.
    pairs = ["()", "[]", "{}"]
    if any(w.count(o) != w.count(c) for o, c in pairs):
        return False
    return not w.endswith(",")

def write(path, name, display, words, trim=()):
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
    head=f'name = "{name}"\ndisplay = "{display}"\n'
    if trim:
        head+="trim = ["+", ".join(json.dumps(t) for t in trim)+"]\n"
    text=head+"words = [\n"+"\n".join(lines)+"\n]\n"
    open(path,"w").write(text)
    print(f"{name}: {len(seen)}")

for name,(display,words,trim) in L.items():
    slug=f"code_{name}"
    write(os.path.join(out,f"{slug}.toml"), slug, f"{display} (code)", words, trim)
    for mname, mdisplay, mwords in M.get(name, []):
        d=os.path.join(out, slug)
        os.makedirs(d, exist_ok=True)
        write(os.path.join(d, f"{mname}.toml"), mname, mdisplay, mwords)
