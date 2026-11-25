# Phản biện 02: người hoài nghi

> Phản biện của hội đồng cho bốn bản nháp `01-strategy.md`, `02-design.md`, `03-ledger.md`, `04-launch.md`. Ngày 2026-11-01.
> Vai: người hoài nghi. Không bàn lại quyết định của owner (big bang trong ranh giới, không thí nghiệm, không bí mật, không giới
> hạn sáng kiến, ba điểm cố định, các rail của `AGENTS.md`). Việc của bản này: chỉ ra chỗ blueprint, **với chính các quyết định
> đó**, tự mâu thuẫn, nói quá, dựa vào bằng chứng yếu, hoặc hỏng theo cách các bản nháp đánh giá thấp.
> Cách trích: `01:62` = `docs/council/01-strategy.md` dòng 62 (tương tự 02, 03, 04); `AGENTS.md:21`; `ADR-0018:40` =
> `docs/adr/0018-*.md` dòng 40. Mã arXiv trỏ tới PDF trong `docs/papers/`; mọi trích dẫn PDF ở đây tôi đã tra trong văn bản trích
> bằng pypdf (venv tạm ở `/tmp`, đã xoá). Nhãn: **HIGH** (đã đọc nguồn trong lần soạn này), **MEDIUM** (suy luận từ nguồn đã đọc),
> **LOW** (phỏng đoán).

## 0. Tóm tắt

Blueprint trung thực hơn đa số tài liệu cùng loại: nó tự nói nhân quả không xác định được (`02:181`) và tổn thất không có cận với
hành động không đảo ngược được (`02:432`). Vấn đề là nó **nói ra rồi không rút hệ quả**: phần khác của chính nó vẫn hứa tổn thất có
cận (`02:246`), vẫn đặt ngưỡng "luận điểm đang sai" như thể quan sát quy kết được (`01:262`), và bốn bản nháp chọn ba lập trường khác
nhau về rail merge. Bốn phản bác dưới đây nhắm vào chỗ người lạc quan ít muốn nghe nhất; mục 7 xếp hạng những gì phải đổi.

## 1. Bốn phản bác mạnh nhất

### P1. Kiểm chứng không chặn thì không còn là "năng lực kiểm chứng"; giá trị co lại thành phát hiện và sửa, chỉ đúng với việc đảo ngược được

- **Luận điểm của blueprint.** Thứ khan hiếm là "biết chắc một kết quả đúng với khách hàng mà không cần người đọc lại" (`01:37-40`).
  Nhưng theo quyết định "không cổng duyệt", tầng kiểm chứng "báo cáo và sinh việc sửa; nó không chặn sáng kiến" (`02:141-142`), kiểm
  release "không bị chặn" (`02:222`, `04:113-116`), taint "ghi và hiển thị, không chặn" (`02:133`). Một con số sai mà checker bắt được
  vẫn tới khách; checker chỉ rút ngắn thời gian tới lúc sửa. **HIGH** (đọc trực tiếp).
- **Bằng chứng.** Chính các nguồn blueprint dựa vào cho thấy lợi ích của kiểm chứng đến từ việc nó **chặn trước khi gộp**: kiến trúc
  tập trung giữ khuếch đại lỗi ở 4,4 lần "by enforcing validation bottlenecks that intercept errors before aggregation", so với 17,2
  lần khi độc lập (2512.08296); Loop-Back kết luận "Authority improves quality when the Manager can verify the work" (2609.14767), tức
  quyền gửi trả có ích khi kiểm chứng được, đúng trường hợp blueprint bỏ đi. `02:366` tự ghi điều này nhưng giữ thiết kế không chặn.
  Với hành động ra ngoài, phục hồi không xoá được hậu quả: PocketOS xoá volume trong 9 giây (`01:247`), Knight lỗ hơn 460 triệu USD
  trong khoảng 45 phút (`01:99`); `02:432-433` thừa nhận "một email, một khoản hoàn tiền thì phục hồi không xoá được".
- **Mâu thuẫn với lập trường của dự án.** `AGENTS.md:21` cho phép nhận rủi ro "when its loss is bounded and recovery is cheap". Với
  lớp hiệu ứng không đảo ngược (gửi ra ngoài, tiền, cam kết, rò dữ liệu qua công cụ hợp lệ), blueprint không chỉ ra cận ở đâu.
  `02:363` tự chấm "LOW rằng tổn thất có cận chỉ nhờ phát hiện và dừng". Vậy với lớp đó điều kiện của `AGENTS.md:21` **chưa thoả**,
  và không bản nào nói thẳng như vậy. **HIGH**.
- **Phản bác tốt nhất.** Phần lớn hiệu ứng của một công ty phần mềm (mã, cấu hình, dữ liệu có backup) đảo ngược được; với chúng,
  phát hiện nhanh cộng revert theo băm là đủ, như bake time của SRE. Agent vẫn tự chọn chạy checker trước khi ship ("Judgement belongs
  to agents", `AGENTS.md:45`). Số khách dựa vào chỉ đổi qua merge (I10, `02:59`), nên lỗi nặng nhất có một chốt.
- **Bản nháp đã trả lời chưa.** Một phần: `02:432-434` và `04:257` nhận yếu điểm. Chưa: không có phân loại hiệu ứng theo khả năng đảo
  ngược, nên người đọc không biết phần nào của "tổn thất có cận" là thật.
- **Cần nói.** Một bảng lớp hiệu ứng (đảo ngược / bù trừ được / không đảo ngược), với cơ chế phục hồi đã xây cho từng lớp, và câu
  thẳng: "với lớp không đảo ngược, rủi ro được nhận mà không có cận; điều kiện của `AGENTS.md:21` không thoả ở đây". Phục hồi xây
  trước (bù trừ, bản ghi trước khi gửi) là đúng lập trường và không giới hạn sáng kiến. **MEDIUM**.

### P2. "Học bằng quan sát" mà không có đối chứng thì không học được cái gì đã sửa đúng

- **Luận điểm.** "Mọi hiểu biết đến từ quan sát vận hành thật" (`01:16`), "sửa trong lúc chạy" (`01:15`), mỗi thay đổi ra ngay
  (`04:111-113`). Cùng lúc, blueprint thừa nhận ROI chỉ là "tương quan theo thời gian, không phải nhân quả" (`01:154`) và "tác động
  nhân quả ròng ... không xác định được" (`02:181`). Thế nhưng S1–S7 được viết là "mức mà nếu kéo dài, luận điểm đang sai" (`01:262`).
  Một ngưỡng trước–sau không thể làm luận điểm sai khi chính bản nháp nói trước–sau bị thời gian làm nhiễu (`01:152-153`). **HIGH**.
- **Bằng chứng.** Chuyên gia của 15 nền tảng: "there is no substitute for a well-run long-term experiment, whether for learning
  surrogates or validating them", và surrogate học từ thí nghiệm "generally preferable to observationally-learned" (2608.08043).
  Surrogate index của Athey cần một mẫu thí nghiệm có gán ngẫu nhiên ("assignment in the experimental sample is Unconfounded",
  athey-2019 NBER w26463); `02:183` đề xuất hiệu chỉnh surrogate bằng kết quả quan sát, ngoài điều kiện nhận dạng của phương pháp đó.
  Bing: lỗi giảm chất lượng làm doanh thu mỗi người dùng +30% (Kohavi 2012). Cái không ai thấy không vào mẫu (`03:399`, U1). **HIGH**.
- **Điểm nhiều người bỏ qua.** Nhân quả không cần thí nghiệm nếu có so sánh **đồng thời**: 2607.01904 dùng staggered
  difference-in-differences trên dữ liệu quan sát (việc dùng AI bắt đầu ở mỗi người một lúc). Big bang xoá đúng sự so le đó.
  Nguồn so sánh còn lại là **ranh giới**: `04:201-203` để doanh nghiệp chọn một tenant hay một đơn vị, "bên ngoài ranh giới, không gì
  thay đổi", tức có nhóm so sánh tự nhiên. Nhưng `01:346-347` định nghĩa ranh giới là toàn bộ chu vi pháp lý của doanh nghiệp, xoá
  nhóm đó. **HIGH** cho mâu thuẫn, **MEDIUM** cho cách dùng ranh giới làm so sánh (không ngẫu nhiên, chịu khác biệt giữa đơn vị).
- **Phản bác tốt nhất.** SRE phát hiện suy giảm hằng ngày mà không cần A/B; vạch release theo `config_hash` (`02:221-223`) và lỗi gieo
  cộng dữ liệu phát lại (`02:184`) là kiểm tra dụng cụ; hiệu ứng lớn hiện ra mà không cần RCT.
- **Bản nháp đã trả lời chưa.** Trả lời trung thực phần "biết", chưa trả lời phần "học": khi nhiều release ra mỗi ngày trên mọi role,
  vạch release chồng lên nhau và không gắn được kết quả với thay đổi. `04:120-122` (một release mỗi role đang chạy hồi quy) giúp ở
  bộ hồi quy, không giúp ở kết quả của khách.
- **Cần nói.** Một chiến lược nhận dạng công bố trước ngày 0 (bên trong / bên ngoài ranh giới, chuỗi thời gian ngắt quãng với ngưỡng
  định trước), và viết lại S1–S7 thành "chưa được ủng hộ nếu" thay vì "đang sai nếu" khi không có so sánh đồng thời.

### P3. "Không bí mật" ăn mòn chính cái hào mà luận điểm gọi là tài sản

- **Luận điểm.** Tài sản chính là bộ kiểm chứng tích luỹ (`01:40`, `01:225-229`). Một bộ kiểm chứng chỉ đo được khi bên bị đo không
  thấy nó: tập neo trong 2606.15474 là "a fixed, human-labeled anchor set A, held out before monitoring begins". Nhưng "không bí mật"
  được định nghĩa theo bốn cách: mọi thứ nhìn thấy được trừ credential (`01:349-350`); mọi dữ kiện kinh doanh mở cho mọi agent
  (`03:33`); không gì ẩn với owner, còn nhãn lỗi gieo ẩn với agent bị kiểm (`02:420-421`); công bố cả bộ hồi quy và bảng mỗi lần chạy
  ra thế giới (`04:172-173`). **HIGH**.
- **Bằng chứng.** Dùng lại holdout dẫn tới "significant overfitting to the holdout" (Dwork et al., 1411.2664); người dự thi Kaggle
  "may begin to overfit to the holdout data that supports the leaderboard" khi được chấm lặp lại (Blum và Hardt, 1502.04585). Ở đây
  agent tự sửa brief và playbook (`04:107`), thấy kết quả hồi quy trên màn hình của mình (`04:113`), và chạy liên tục: đúng cấu trúc
  phân tích thích ứng. Model tối ưu mạnh lên detector thì học che giấu (2503.11926: "agents learn obfuscated reward hacking"); judge bị
  lừa bằng token hời hợt tới 80% FPR (2507.08794, `03:94`). `04:182-184` chỉ nêu rủi ro nhiễm dữ liệu huấn luyện, không nêu agent
  của chính hệ thống quá khớp với bộ kiểm của nó. **HIGH** cho cơ chế, **MEDIUM** cho mức độ trong hệ này (chưa đo).
- **Phản bác tốt nhất.** `02:420-421` đã giữ nhãn lỗi gieo ẩn với agent; bộ hồi quy liên tục thêm ca mới (`04:183-184`); hào là năng
  lực hiệu chỉnh lại, không phải bộ ca cố định (`01:233`).
- **Bản nháp đã trả lời chưa.** Một phần, và bằng một ngoại lệ chưa được owner phê: `02:420` tạo ra một bí mật (nhãn lỗi gieo) trong
  khi quyết định nói "không bí mật". `03:33` và `04:173` đi ngược ngoại lệ đó.
- **Cần nói.** Một định nghĩa duy nhất của "không bí mật" và một ngoại lệ thứ hai được gọi tên như credential: **tập đánh giá niêm**
  (neo, nhãn lỗi gieo, một lát của bộ hồi quy) không vào ngữ cảnh agent bị đo, owner thấy hết. Không có nó thì mọi số M1–M3 là số
  lạc quan, và phải ghi như vậy.

### P4. Một nút dừng thủ công không phải là kiểm soát của con người, theo cả bằng chứng lẫn luật mà F3 bắt tuân theo

- **Luận điểm.** Sau khi bỏ cổng, F1 là kiểm soát duy nhất do người giữ (`03:249`). Thời gian dừng có thiết kế (I5, D1), còn **thời
  gian phát hiện**, đại lượng chi phối tổn thất (`02:29-31`), không có mục tiêu, không có cận, và do agent (auditor, tripwire) giữ.
- **Bằng chứng.** 94% lập trình viên không phát hiện phá hoại; có monitor báo đúng vẫn thành công 9/16 (2606.05647). Chuyển ca đã gắn
  cờ cho người giúp TPR khoảng 15% ở FPR 0,01, nhưng cùng bài kết luận "lack of adversarial robustness for LLMs and humans"
  (2508.19461); `02:237` chỉ trích nửa đầu. Giải thích làm người chấp nhận khuyến nghị "regardless of its correctness" (2006.14779).
  Khảo sát 41 chính sách: yêu cầu giám sát của người tạo "a false sense of security" (Green, 2109.05067).
- **Luật.** `01:284` viết "nút dừng của owner cũng là nghĩa vụ pháp lý", như thể nút dừng là đủ. `03:379` lại hỏi luật sư đúng câu đó,
  và `03:366` ghi Quyết định 33/2026 xếp rủi ro cao hệ AI "automatically perform high-value electronic transactions … without human
  inspection, approval or control before execution", kèm Nghị định 142 đòi giám sát con người và đánh giá sự phù hợp **trước triển
  khai** (MEDIUM, nguồn hãng luật). `03:276-277` nhận "F3 có thể buộc chính những thứ owner đã bỏ". Vậy "không cổng duyệt" (`01:18`,
  `02:19`) và F3 có thể va nhau, và blueprint không nói bên nào thắng. Vì F3 là điểm cố định, **F3 thắng**; văn bản phải ghi thế.
- **Phản bác tốt nhất.** Owner không phải lưới chặn (`AGENTS.md:53`); bằng chứng nói duyệt từng việc là tệ (`02:436`), nên dồn người
  vào hàng đợi ngoại lệ là đúng hướng (`02:236-237`). Dừng ở gateway giải quyết Palisade (2509.14260).
- **Bản nháp đã trả lời chưa.** `03:435-436` chấm LOW và `03 §3.4` liệt kê câu hỏi; `01` và `02` không mang kết quả đó vào luận điểm.
- **Cần nói.** Mục tiêu thời gian phát hiện theo lớp hiệu ứng, đo bằng lỗi gieo trước ngày 0; quy tắc ưu tiên "F3 > không cổng";
  các câu `03:371-390` là điều kiện để **bật chức năng tương ứng**, không phải câu hỏi mở sau khi bật. Đây là tuân thủ, không phải giai
  đoạn.

## 2. Mâu thuẫn nội bộ

| # | Mâu thuẫn | Chỗ | Hệ quả |
|---|---|---|---|
| M1 | Rail merge: `01:28-30` nói quyết định "không giữ merge"; `02:26`, `02:197` và `04:36-39` giữ merge; `03:36-37` không chọn. Ba lập trường trong bốn bản | `AGENTS.md:47-49` | Không bản nào đúng với hai bản còn lại; người đọc không biết code có vào production mà không qua owner hay không |
| M2 | Giữ merge mà bỏ trần ADR-0001 (N = 5, "a rail: no agent raises it", `ADR-0001:20,28`): hàng đợi của owner không có cận, đúng hiện tượng `01:51` nêu (52 MR chờ) | `01:29`, `02:376`, `02:436` | Owner thành cổng duyệt cho mọi thay đổi code, đúng vai bằng chứng nói người làm kém (93% phê duyệt) |
| M3 | ADR-0010 giữ các giới hạn đếm (`-max-waiting-mrs`, `-max-concurrent`...) để bảo vệ sự chú ý của owner (`ADR-0010:31`); blueprint nói không giới hạn | `01:18-19`, `02:19-20` | Phải nói giới hạn đếm này là rail hay là giới hạn bị bỏ |
| M4 | `02:51` và `02:195` lấy ADR-0018 làm hiện thực tham chiếu nhưng bỏ bất biến "Least grant" (grant trên `read` chờ owner, `ADR-0018:40-43`) và ghi "mọi agent dùng được mọi connection". ADR-0018 nói "A way of acting that breaks one is not offered" (`ADR-0018:36`) | `02:195` | Hoặc sửa ADR-0018, hoặc không dẫn nó làm cơ sở; hiện là dẫn chọn lọc |
| M5 | Đặc tả tự mâu thuẫn: I1 nói gate "chỉ từ chối vì F1 hoặc F3" (`02:50`), `Resolve` chỉ nhận `StopState` và `LegalPolicy` (`02:320`), và property test đòi kết quả `execute` cho mọi hành động không bị dừng và hợp luật (`02:348-349`). Nhưng I4 từ chối mọi công cụ ghi vào log và backup (`02:53`), I10 và I16 từ chối đổi mã số liệu và mã rail ngoài merge (`02:59`, `02:65`) | `02:50-65`, `02:320-349` | Property test sẽ hỏng ngay khi I4 hay F4 được hiện thực; cần một tập lý do từ chối đóng gồm F1, F3, F4, I4 |
| M6 | `03:182` (R0): "Ngày 0: mọi role có quyền ghi production"; `02:59` (I10): số khách dựa vào chỉ đổi qua merge | `03:182`, `02:59` | Hai bản mô tả hai hệ khác nhau ở đúng rủi ro R1 |
| M7 | Không thí nghiệm và lặp eval: `03:282` "không có lặp nhiều lần hay pass^k vì đó là thí nghiệm"; `02:162-163` "Lặp eval trên tác vụ ghi lại không phải thí nghiệm"; `04:61` và `04:141` đòi K ≥ 5 và pass^k | `03:282` | Phải chọn một định nghĩa "thí nghiệm"; bản `02:417` là định nghĩa dùng được |
| M8 | Không thí nghiệm mà "học bằng quan sát": `01:16` so với `01:154`, `02:181`; S1–S7 viết như phép bác bỏ (`01:262`) | P2 | Ngưỡng "đang sai" không có so sánh đồng thời là lời hứa không giữ được |
| M9 | Ranh giới: toàn doanh nghiệp (`01:188`, `01:346-347`) hay một tenant hay đơn vị (`04:201-203`) | P2 | Định nghĩa của `04` để lại nhóm so sánh; định nghĩa của `01` xoá nó |
| M10 | "Không bí mật": bốn định nghĩa (`01:349`, `02:420-422`, `03:33`, `04:170-177`); `03:389-390` hỏi nó có trái tối thiểu hoá dữ liệu (GDPR Điều 5) không | P3 | Mở mọi dữ liệu cá nhân của người mua cuối cho mọi agent có thể trái F3; F3 thắng |
| M11 | Không giới hạn sáng kiến và F3: Quyết định 33/2026, Nghị định 142 (`03:365-366`), AI Act Điều 14 (`03:338`) | `01:18`, `01:284`, `03:371-373` | Thiếu quy tắc ưu tiên |
| M12 | Lập trường rủi ro: `AGENTS.md:21` đòi tổn thất có cận; `02:246` hứa "không thiếu thứ gì mà thiếu nó thì tổn thất ở ngày 0 không có cận"; `02:432-433` thừa nhận lớp không đảo ngược không có cận | P1 | `02 §3` nói quá điều `02 §9` thừa nhận |
| M13 | ADR-0010 tự ghi "Revisit when ... runs for someone other than its owner" (`ADR-0010:43`); blueprint cho doanh nghiệp lớn chính là trường hợp đó, mà `01:147` vẫn dựa vào ADR-0010 | `01:147-150` | Điều kiện xét lại của ADR đã đến; blueprint phải nói ai đọc chi phí từ ngày 0 |
| M14 | ADR-0013: "The check is judged before the work starts, by someone other than its author" (`ADR-0013:37`); `02:107`, `02:111-112`: không ký duyệt, thực thi không chờ | `02:107` | Hoặc đó là một cổng (đối với thước đo) và phải gọi tên, hoặc ADR-0013 đổi |
| M15 | Số liệu: `01:63-64`, `01:297` và `03:62` ghi "độ chính xác tăng 0,18 và 0,33"; `04:280-281` đính chính. PDF: 0,18 và 0,33 là độ dốc của reliability theo accuracy; accuracy tăng 0,23/năm và 0,22/năm (2602.16666, Hình 1 và phụ lục) | 2602.16666 | Kết luận giữ nguyên, số sai ở hai bản dù bản thứ tư đã sửa |

## 3. Khẳng định không bác bỏ được, và cách viết lại

| Chỗ | Nguyên văn (rút gọn) | Vì sao không bác được | Viết lại có thể sai |
|---|---|---|---|
| `01:37-40` | Công ty thu lợi khi giao toàn bộ cho agent, agent tự xây kiểm chứng, coi bộ kiểm chứng là tài sản | Không có mốc, không có so sánh | "Ở lớp việc có checker xác định, chi phí trọn gói mỗi đơn vị đứng vững tháng 7–9 thấp hơn ≥ X% so với cùng lớp việc bên ngoài ranh giới cùng kỳ; nếu không, luận điểm không được ủng hộ ở lớp đó" |
| `01:162-163` | Công ty chờ sẽ có model giỏi mà không có gì để biết chắc | Câu tu từ; `01:234` lại nói đối thủ tự xây được | Xoá, hoặc: "thời gian để một công ty mới dựng bộ kiểm chứng đạt d′ ≥ D trên lỗi gieo là T tháng", rồi đo T |
| `01:165` | Bật một lần tăng tốc tích luỹ kiểm chứng | Không định nghĩa "tăng tốc" (đã LOW) | "Số ca hồi quy mới từ lỗi thật mỗi tuần mỗi lớp việc, so với đơn vị bật sau" |
| `01:218` | "Hệ thống vận hành để biết chắc" | Khẩu hiệu | Bỏ, hoặc gắn vào DER_30 (`04:143`) với ngưỡng |
| `01:233` | Hào là năng lực hiệu chỉnh lại liên tục | Không có thước đo | "Từ lúc judge đổi phiên bản tới lúc tập neo báo và panel được hiệu chỉnh lại ≤ N ngày, đo ở mọi lần đổi" |
| `01:286` | Vận hành thật tạo ra dữ liệu chưa ai có | Đúng một cách tầm thường | Nêu câu hỏi cụ thể dữ liệu đó trả lời và cách nhận dạng (P2) |
| `02:235` | "Owner không cần đọc nó để hệ thống an toàn" | "An toàn" không định nghĩa | "Với mọi lỗi gieo mức C0, ngoại lệ tới hàng đợi trong ≤ T phút mà owner không mở màn hình agent" |
| `02:297` | Agent kháng tắt "không có đường ra sau khi gate đóng" | Khẳng định phổ quát | "Trong N diễn tập đường ra trên mọi hình thức (tool, CLI trong jail, MCP, trình duyệt), 0 byte ra không có bản ghi gate sau `STOPPED`" (I1 đã gần như thế; nên nói là kết quả đo, không phải sự thật) |
| `04:21` | Phục hồi "quyết định một sai lầm tốn bao nhiêu" | Sai với lớp không đảo ngược | "Với lớp hiệu ứng đảo ngược, cửa sổ mất dữ liệu ≤ W và thời gian khôi phục ≤ R trong diễn tập; lớp không đảo ngược không có cận" |
| `04:191` | Scorecard thành tiêu chuẩn chung | Không có tiêu chí thành công | "Sau 12 tháng có ≥ K tổ chức ngoài công bố M1–M7 theo cùng định nghĩa" |
| `01:262-273` | S1–S7: "luận điểm đang sai nếu" | Không có đối chứng thì không quy được (`01:152`) | "Luận điểm không được ủng hộ nếu ... so với bên ngoài ranh giới cùng kỳ"; với S7 giữ nguyên (một ca là đủ) |

## 4. Pre-mortem: một doanh nghiệp lớn dùng blueprint và thất bại trong sáu tháng

Giả định: doanh nghiệp nhiều đơn vị, có pháp chế, tài chính, công đoàn và hệ thống cũ không qua gate. Xếp theo khả năng tôi đánh giá
(**MEDIUM** cho thứ tự; mỗi nguyên nhân có nguồn).

| # | Nguyên nhân | Tín hiệu sớm sẽ thấy | Blueprint lẽ ra phải khác ở đâu |
|---|---|---|---|
| 1 | Hàng đợi của owner sập: merge vẫn là rail, không còn trần, hàng trăm MR chờ mỗi owner; owner duyệt máy móc hoặc ngừng duyệt (52 MR ở n = 1, ADR-0001; tải review gấp đôi, 2607.01904; 93% phê duyệt, `02:232`) | M5: tuổi p95 của việc chờ và tỷ lệ merge không bình luận tăng từ tuần 1 | Giải M1–M3 ở mục 2: một quy tắc cho merge và trần, không phải ba |
| 2 | Không trả lời được "có hiệu quả không" ở tháng 4–6: chi phí hoá đơn tăng, không quy được kết quả, nhà tài trợ cấp điều hành rút (`01:154`, `02:181`; 2608.08043) | Vạch release dày tới mức không đọc được; S1–S2 mơ hồ; tài chính hỏi số mà hệ không ghi (ADR-0010) | Chiến lược nhận dạng và người đọc chi phí công bố trước ngày 0 (P2, M13) |
| 3 | Pháp chế dừng hoặc buộc gắn cổng sau khi bật: chức năng chi tiền hoặc hoàn tiền rơi vào Quyết định 33/2026; mở mọi dữ liệu cá nhân cho mọi agent trái tối thiểu hoá (`03:366`, `03:389`) | Câu `03:371-390` chưa có câu trả lời ký ở ngày 0; DPIA chưa xong | F3 thắng "không cổng"; câu trả lời của luật sư là điều kiện bật từng chức năng (P4) |
| 4 | Confused deputy: injection qua nội dung khách hoặc web khiến agent dùng connection hợp lệ để gửi dữ liệu hoặc cam kết ra ngoài; không đảo ngược được (2510.09023 >90% vượt phòng thủ tĩnh; 2503.12188 58–90%; `02:212-214` bỏ Rule of Two và CaMeL) | Tripwire "đích mới" và "phiên untrusted lên connection nhạy cảm" tăng mà không ai xử lý (W8) | Phân loại hiệu ứng (P1); nói thẳng đây là rủi ro lớn nhất mang ở ngày 0 (`04:257` đã nói, `01` thì không) |
| 5 | Gate không phủ hết: doanh nghiệp có SaaS, phiên SSO, email, hệ cũ; một đường ra ngoài gate làm F1 không dừng được hết và log không đầy đủ | Diễn tập I1 tìm thấy byte ra không có bản ghi; hiệu ứng ở hệ ngoài không có dòng log (D4) | I1 và D4 phải đạt trên **toàn bộ** ranh giới trước khi bật, và ranh giới chỉ gồm hệ thống gate phủ được |
| 6 | Số sai âm thầm tới khách: auditor là LLM cùng kiểu lỗi, phát hiện sau khách (C28–C31; FinSheet 82,4%, 2603.07316) | Khiếu nại của khách tới trước báo cáo của auditor (U2) | Tỷ lệ bắt của auditor đo bằng lỗi gieo **trước** ngày 0, có ngưỡng; hiện là "tinh chỉnh" (`02:260`) |
| 7 | Scorecard xanh, khách đỏ: agent quá khớp với bộ hồi quy và tập neo mở (P3; 1502.04585; Bing, Kohavi 2012) | Tỷ lệ đạt hồi quy tăng trong khi D10 (khiếu nại, mở lại) phẳng hoặc xấu đi | Tập đánh giá niêm là ngoại lệ có tên của "không bí mật" |
| 8 | Chi phí bùng mà không ai ghi: multi-agent khoảng 15 lần token chat (`01:120`), vòng meltdown (2502.15840), không trần, không sổ | Hoá đơn tháng tăng nhanh hơn số kết quả đã xác minh | Người đọc chi phí ngoài vòng lặp agent là thành phần ngày 0 có chủ, không chỉ đề xuất (`01:148`) |
| 9 | Nợ và xung đột: nhiều agent, mọi connection, ghi chồng; độ phức tạp +41% kéo dài (2511.04427); xung đột 27,67% PR (2604.03551) | M4 tăng tuần qua tuần; tỷ lệ làm lại 30 ngày tăng | W2 và W12 có, nhưng chỉ "đổi brief"; cần ngưỡng đọc định trước |
| 10 | Phản ứng của người lao động và khách: chức năng bị thay quay lại qua kênh khác (CBA, Klarna, `04:242-243`) | D10: khối lượng cuộc gọi, leo thang | `04:236` tham vấn đại diện người lao động "trước khi thay đổi vị trí"; big bang khiến việc này trùng ngày 0, cần nói rõ thứ tự |

Nhận xét về pre-mortem của `04:247-263`: mỗi dòng ở đó đều có biện pháp; không dòng nào là nguyên nhân 1, 2, 3 hay 7 ở trên. Một
pre-mortem chỉ liệt kê điều đã có lời giải dễ đánh giá thấp chính những thất bại không có lời giải. **MEDIUM**.

## 5. Chỗ tự tin vượt bằng chứng

| Chỗ | Nguyên văn | Bằng chứng thật nói gì |
|---|---|---|
| `01:194` | "Đến sớm: kỹ thuật phần mềm, việc có test, bài toán phân rã được (+80,8% với kiến trúc tập trung) [... HIGH]" | +80,8% là trên Finance Agent, "decomposable financial reasoning"; cùng bài: "SWE-bench Verified shows slight degradation across all MAS architectures" (2512.08296). Con số đặt sai miền, và miền tài chính lại bị xếp "đến muộn" ở `01:195` |
| `01:227`, `02:158`, `03:67`, `04:84` | Tập neo "phát hiện 60/60 lần đổi phiên bản judge" | Bài có hai thay đổi judge thật; **một** lần đổi phiên bản âm thầm được phát hiện trong 60/60 lần chạy; tác giả nêu "design law" là neo phải chạy nhanh hơn tiến trình chính (2606.15474). Đây là 60 lần lặp của một sự kiện, một tác giả |
| `01:63-64`, `01:297`, `03:62` | "độ chính xác tăng 0,18 và 0,33" (HIGH) | Sai đại lượng (M15) |
| `01:130`, `02:395-396` | Lợi ích đầu kỳ tan sau hai tháng, nên đọc từ tháng ba | 2607.01904 (cũng được dẫn) cho thấy thông lượng tăng dần, đạt 2× "only after roughly nine months' usage". Hai nguồn kéo hai hướng; khung sáu tháng của S1 (`01:267`) có thể kết luận sớm |
| `01:174`, `02:121-122` | Reviewer chỉ góp ý làm xấu kết quả (HIGH) | Một nghiên cứu, 43 cặp sản phẩm, 86 lần chạy, một loại việc (2609.14767); HIGH cho số đo, MEDIUM cho khái quát |
| `01:284` | "nút dừng của owner cũng là nghĩa vụ pháp lý" | Có thể là cần, chưa biết là đủ (`03:379`; `03:366`) |
| `02:237` | Kale: TPR +15% ở FPR 0,01 | Cùng câu tóm tắt: "lack of adversarial robustness for LLMs and humans" (2508.19461) |
| `02:246` | "không thiếu thứ gì mà thiếu nó thì tổn thất ở ngày 0 không có cận" | `02:432-433`, `02:363` (LOW) |
| `02:260` | Panel auditor là tinh chỉnh vì "không làm tổn thất mất cận khi tầng 0 có mặt" | `02:438`: role thiết kế và sản phẩm gần như không có checker xác định, nên tầng 0 thường vắng |
| `02:262` | Surrogate sai "làm đọc KPI sai, không làm hành động mất cận" | Agent tối ưu theo KPI; KPI sai dẫn hành động (Goodhart, 2210.10760; Sigerson 2608.08043 về khai thác bằng thông báo) |
| `04:47` | "Một thành phần thiếu ở ngày 0 không bao giờ có dữ liệu nền để so sánh" | Dùng để biện minh cho big bang; nhưng dữ liệu nền cần là **trước** ngày 0, và chính big bang xoá nhóm so sánh đồng thời (P2) |
| `04:257` | Injection: "backup và hoàn tác sửa hậu quả" | Backup không lấy lại dữ liệu đã rò hay email đã gửi; cùng dòng tự nói "không có biện pháp nào chặn hành động trước khi nó xảy ra" |

## 6. Điều blueprint làm đúng và nên bảo vệ

1. **Tách niềm tin và nghi ngờ.** Sổ claim có nhãn (`01:288-341`), sổ kiểm lại tìm ra số đổi mẫu số (`03:153-167`). Hiếm thấy, và
   đúng `AGENTS.md:28`.
2. **F2 niêm credential trong executor** (`02:195`, `04:56`): có bằng chứng sự cố thật (PocketOS) và có phép thử canary (I2).
3. **Dừng ở gateway, ngoài môi trường agent** (I5, D1): đọc đúng Palisade (2509.14260), nơi script tắt nằm trong môi trường model.
4. **Log, ledger, backup ngoài tầm mọi công cụ** (I4, D3) và diễn tập khôi phục đo thời gian: điều kiện để "sửa khi chạy" có nghĩa.
5. **Số khách dựa vào do mã có phiên bản tính** (I10, D9): đúng với FinSheet-Bench (2603.07316).
6. **Verdict phải có `evidence_ref` do nền tảng cấp** (I6) và không có vòng thương lượng (I8): đúng hướng với 2310.01798 và 2603.16244.
7. **Đo độc lập của judge thay vì giả định** (I9, n_eff trên tập neo): đúng 2609.22512 và 2605.29800.
8. **Detector không vào thưởng, mục tiêu tự đặt không vào thưởng** (I11, `02:186-188`): đúng 2503.11926.
9. **Sổ release theo `config_hash`** (I12): đúng bài học hai hậu kiểm của Anthropic.
10. **Owner không làm lưới chặn; hàng đợi ngoại lệ máy xếp hạng; nguồn dữ liệu độc lập cho chức năng bị thay** (D10). Đây là đọc
    đúng bằng chứng tiêu cực về giám sát của người.
11. **Thừa nhận thẳng** rằng không thí nghiệm thì nhân quả không xác định (`02:178-184`) và không có Rule of Two thì bề mặt injection
    mở (`02:365`). Đừng bỏ các câu này khi biên tập cho gọn; chúng là lý do một người phê bình nghiêm túc tin phần còn lại.
12. **Công bố kết quả xấu như kết quả tốt** (`04:175`) và báo cáo lần chạy tệ nhất (`04:150-151`).

## 7. Phán quyết: phải đổi gì trước khi công bố (xếp hạng)

1. **Một bộ rail, một nguồn sự thật.** Chọn một lập trường về merge, trần MR (ADR-0001), giới hạn đếm (ADR-0010), Least grant
   (ADR-0018) và kiểm thước đo trước khi làm (ADR-0013); sửa `AGENTS.md` hoặc các ADR trong cùng thay đổi; sửa đặc tả `Resolve` cho
   tập lý do từ chối đóng (F1, F3, F4, I4). Không có việc này thì `02 §5` không hiện thực được như viết (M1–M5, M14). **HIGH**.
2. **Phân loại hiệu ứng theo khả năng đảo ngược** và ghi cận tổn thất cho từng lớp; với lớp không đảo ngược, ghi thẳng điều kiện của
   `AGENTS.md:21` không thoả và rủi ro được nhận không có cận; sửa `02:246` và `04:21` cho khớp `02:432` (P1, M12). **HIGH**.
3. **Quy tắc ưu tiên "F3 thắng"** và câu trả lời ký của luật sư cho `03:371-390` là điều kiện bật từng chức năng; bỏ câu `01:284`
   hoặc hạ nó thành câu hỏi (P4, M10, M11). **HIGH** cho mâu thuẫn, **MEDIUM** cho nội dung luật (nguồn thứ cấp).
4. **Định nghĩa "không bí mật" một lần**, với tập đánh giá niêm là ngoại lệ có tên ngoài credential; không công bố toàn bộ bộ hồi
   quy (`04:173`), hoặc ghi rằng mọi số M1–M3 là cận trên lạc quan (P3). **HIGH** cho cơ chế.
5. **Chiến lược nhận dạng trước ngày 0** (so sánh bên trong / bên ngoài ranh giới, chuỗi thời gian ngắt quãng với ngưỡng định trước),
   dùng định nghĩa ranh giới của `04:201-203` thay `01:346-347`; viết lại S1–S7 (P2, M8, M9, mục 3). **MEDIUM**.
6. **Sửa số sai**: độ dốc reliability (`01:63-64`, `01:297`, `03:62`), 60/60 ở cả bốn bản, +80,8% (`01:194`), một định nghĩa thí
   nghiệm (`03:282` theo `02:417`), phần sau của Kale (`02:237`) (M7, M15, mục 5). **HIGH**.
7. **Đặt đường đọc theo cả hai nguồn**: 2511.04427 (tan sau hai tháng) và 2607.01904 (2× sau khoảng chín tháng dùng); đừng để khung
   sáu tháng kết luận thay cho dữ liệu. **HIGH** cho nguồn, **MEDIUM** cho hệ quả.
8. **Chi phí có người đọc từ ngày 0**: điều kiện xét lại của ADR-0010 đã đến với doanh nghiệp khác owner; ghi ai đọc hoá đơn, theo
   lớp việc, mở cho mọi người, ngoài thưởng (M13). **HIGH** cho mâu thuẫn.
9. **Thời gian phát hiện có mục tiêu** theo lớp hiệu ứng, đo bằng lỗi gieo trước ngày 0; tỷ lệ bắt của auditor lên hạng "ngày 0"
   thay vì "tinh chỉnh" (`02:260`) (P4, pre-mortem 6). **MEDIUM**.
10. **Gate phủ toàn ranh giới là điều kiện của ranh giới**: hệ thống không đi qua gate nằm ngoài ranh giới (pre-mortem 5). **MEDIUM**.
11. **Bổ sung pre-mortem của `04`** bằng các nguyên nhân không có lời giải (mục 4: 1, 2, 3, 7). **MEDIUM**.

## Giả định

- Assumption: "the blueprint" is the four council drafts read together, because the final report will merge them and a reader will
  meet their contradictions side by side.
- Assumption: a deterministic check that refuses (I4, I10, I16, F4 merge) is a rail and not a "limit on initiative", because
  `AGENTS.md:47-51` names rails as deterministic code the owner pulls; the drafts need to state this rule explicitly (M5).
- Assumption: the legal sources at MEDIUM in `03 §3` (Quyết định 33/2026, Nghị định 142/2026) say what the law-firm summaries say,
  because I did not read the official texts; P4 and pre-mortem 3 depend on it.
