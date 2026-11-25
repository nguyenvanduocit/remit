# Blueprint: công ty mà agent AI làm phần lớn công việc (chiến lược, thiết kế, bằng chứng)

> Bằng chứng và phương pháp vận hành đứng sau thiết kế của Remit; thiết kế dựng từ nó là [`remit-design.md`](../remit-design.md). Remit là sự tiến hoá của Remit: cùng một codebase, đưa sang thiết kế mới; code, module Go
> và binary vẫn mang tên `remit` cho tới khi đổi tên (văn bản này gọi hiện thực tham chiếu là Remit). Hôm nay bên áp dụng là owner cùng một team khác; chưa có khách, phân phối ra
> ngoài HOLD (`remit-design.md` K11). Hội đồng 2026-11-01; hồ sơ ở [`council/`](council/) (mục 12). Không phải tư vấn pháp lý. Nhãn: **HIGH** = văn bản gốc đã được người
> đưa ra khẳng định hoặc một kiểm toán viên độc lập kiểm; **HIGH[Cxx]** = chỉ một người phản biện (challenge-xx) đọc văn bản gốc, hội đồng chưa mở lại;
> **MEDIUM** = nguồn thứ cấp hoặc vendor; **LOW** = suy luận. **[BT]** = biên tập tự tra lại trong PDF, ADR hoặc mã (pypdf, venv tạm `/tmp`, đã xoá). Điều không
> có bằng chứng ghi **Đề xuất (giả thuyết)**. [E], [D], [R], [P] = `research-ai-autonomous-system-{evidence,design,risk,pilot}.md`; [C01]–[C06] = `council/challenge-0N-*.md`.

## 0. Kết luận cho bên áp dụng, và executive summary (English)

**Kết luận cho bên áp dụng** (cách hội đồng đọc bằng chứng; mặc định của chủ tọa, chờ owner). Thứ một doanh nghiệp lấy được trước là **lớp đo và kiểm
chứng**: checker xác định cho con số khách dựa vào, tỷ lệ bắt lỗi gieo của checker và auditor, độ độc lập của judge trên tập neo của chính mình, pass^k,
tỷ lệ lỗi thoát 30 ngày (mục 3.5, 6.2, 6.4). Lớp này chạy trên nền tảng agent đã mua và không đòi bỏ giới hạn quyền hay cổng duyệt đang có. Bắt đầu ở
một đơn vị kinh doanh chỉ có hiệu ứng H0 (đảo ngược được trong nền tảng), sau khi luật sư trả lời câu hỏi 1–3 ở mục 4.3 và, nếu dùng Remit, sau
khi mọi lỗ ở mục 7.2 đóng có biên bản diễn tập. Lập trường "gate không giới hạn sáng kiến" (mục 3.7) là **lựa chọn của owner**, tách được khỏi phương
pháp và **chưa được chứng minh**; nó không phải kết luận của bằng chứng.

**Bottom line for an adopter** (the council's reading of the evidence; a chair default pending the owner). What an adopter can take first is the
**measurement and verification layer**: deterministic checkers for customer-facing numbers, seeded-fault catch rates of checkers and auditors, judge
independence on its own anchor set, pass^k, 30-day escape rate (§3.5, §6.2, §6.4). It runs on agent platforms already bought and does not require
removing existing permission scopes or approval gates. Start in a business unit whose effects are all H0 (reversible inside the platform), after counsel
answers §4.3 questions 1–3 and, on Remit, after every hole in §7.2 is closed with a drill record. The "no limits on initiative" gate stance (§3.7)
is the owner's choice, separable from the method and unproven; it is not what the evidence concludes.

**What this is, and what it is not.** An operating method for a company, or one unit of it, that hands most of its work to AI agents: how work is
decomposed, checked, observed and corrected while it runs; which deterministic rails stay with people; what the law forces back; with a reference
implementation (Remit) whose known gaps are day-0 build items, not facts. It is not evidence of profit: no class A or B case of verified profit
exists in the sources searched [P §3], and in CEO-Bench every model ends below a rule-based baseline (2606.18543, HIGH). It is not a replacement for
enterprise agent platforms, and not legal advice. **Return on investment cannot be computed today** (§2.4).

**The evidence in three lines.**
1. Capability grows faster than reliability: the length of software tasks agents finish at 50% doubles about every 7 months (2503.14499), while over
   15 models and 24 months reliability rose 0.03–0.09/year against accuracy 0.22–0.23/year (2602.16666). HIGH.
2. The bottleneck has moved to review: at one software company (panel of 802 developers) AI throughput reached 2× only after about nine months of
   use and load per reviewer doubled (2607.01904); 83 of 88 developers without a monitor missed agent sabotage (2606.05647). HIGH.
3. Checks built from model judgement fail together: three frontier judges from different providers make the same error at 7.7× the independent rate
   (2609.22512); 80.1% of outcome passes in a support benchmark violate the required process (2610.25954). HIGH.

**What is borrowed, and where it departs.** The thesis "buy generation, build verification" is published (Wei's verifier's law; Karpathy's
generation-verification loop; DeepMind's contract-first decomposition, 2602.11865). No component is an invention [C06 §2]. One design choice departs
from every governance framework the prior-art review found (IMDA, OpenAI, Google, NIST NCCoE, OWASP; 35 searches, coverage rather than proof): the
gate refuses an agent's action only for the owner's stop, the law, the fixed rails (the platform's own records among them) and, while ADR-0018's least
grant stays, a missing grant, and the design adds no autonomy levels, spend caps or approval gates of its own. What still waits is forced by law (purpose register for personal data, signed filings) or by the
rails (the owner's merge), plus an isolated reader that holds no action tools; whether the reference's queue cap (ADR-0001), least grant (ADR-0018)
and pre-work check (ADR-0013) stay is open (§11, decisions 2–4). The possible contribution is field data from one organisation on catch rates,
judge independence and 30-day escape rates, beyond the 86-system survey of deployed (production or pilot) agents (2512.04123).

**Three fixed points:** the owner's manual stop, outside anything an agent can change, measured at the host network; credentials never in an agent's
context; legal compliance, where the law wins. The rails (authentication, pause and stop, merge by the owner) stay by default (R1).

**Weaknesses kept visible.** Irreversible effects (an email sent, money paid out, data disclosed, a binding promise) have no loss bound: detection, stop
and recovery limit the next one, not this one. That follows from "no limits" plus irreversibility and conflicts with "take a risk when its loss is
bounded" (`AGENTS.md:21`); the owner must choose how to resolve it (decision 11). Causal effects can only be estimated against units outside the
boundary. The reference does not yet meet fixed points 1 and 2 (ADR-0018:71–72, 112–114; `sandbox_darwin.go:143`).

**Decisions needed** (§11; each has a chair default; the owner has recorded one, item 5: closed source, distribution on HOLD): merge rail; ADR-0001 queue cap (keep);
ADR-0018 least grant; ADR-0013 pre-work check; publication outside the company; instrument measurement as observation; control/data separation (on); hold-and-cancel (off; red team, CRO and cold reader recommend on for some
H2 actions); unbounded H2 loss against `AGENTS.md:21`; banks; cost reading; Vietnam AI Law Art. 4(2); residual-risk signatures; R1–R10.

## Mục lục

0. Kết luận, summary · 1. Khẳng định · 2. Bài toán, ROI · 3. Thiết kế · 4. Luật · 5. Bảo mật · 6. Ra mắt · 7. Remit · 8. Bằng chứng · 9. Rủi ro · 10. Chưa biết · 11. Quyết định · 12. Hồ sơ

**Khung.** Quyết định của owner (nguyên văn: "không có thử nghiệm gì hết, làm là chạy luôn, không có bí mật hoặc bất cứ giới hạn nào hết"): hệ thống bật
một lần bên trong ranh giới bên áp dụng chọn và được sửa trong lúc chạy; không thí nghiệm trên khách; không bí mật; không giới hạn sáng kiến của agent.
Điểm cố định: **F1** dừng tay, **F2** credential ngoài ngữ cảnh agent, **F3** tuân thủ pháp luật. Cách đọc cụ thể của các quyết định đó là phán quyết
R1–R10 của chủ tọa, **mặc định chờ owner xác nhận** (bảng từ ngữ dưới, mục 11). Khi luật đòi một việc trước khi dùng (hồ sơ, test cho chức năng rủi ro
cao, câu trả lời của luật sư), chức năng tương ứng chỉ bật khi việc đó xong: luật thắng "bật một lần" ở chức năng đó (R4).

| Từ | Nghĩa trong bản này |
|---|---|
| Owner | Người có tên chịu trách nhiệm một ranh giới ở bên áp dụng (không phải tác giả Remit); trong Remit là người giữ khoá owner của server, không có người dùng hay vai trò khác (ADR-0026) |
| Đơn vị kết quả / đơn vị kinh doanh | Một kết quả agent giao (một MR, một báo cáo) / một bộ phận của công ty; không dùng "đơn vị" trơn |
| Ranh giới | Đơn vị kinh doanh bên áp dụng chọn để bật hệ thống, chỉ gồm hệ thống mà mọi tác dụng của agent đi qua gate |
| Chain, Chair, Assistant | Một việc từ yêu cầu tới hiệu ứng đã kiểm; agent điều phối một chain; agent nhận lời owner và kéo rail theo tin nhắn đã xác thực của owner |
| Cell, tenant | Một tiến trình, một thư mục dữ liệu, một OS user (`remit-design.md` K3): một cài đặt hay một đơn vị kinh doanh muốn tách, có dữ liệu, secret và nút dừng riêng |
| Rail | Mã xác định bảo vệ owner, không phải hành vi agent: xác thực, pause và stop, merge (`AGENTS.md:50–54`); gọi chung là F4 |
| Điểm cố định | F1 dừng tay, F2 credential, F3 luật; không quyết định nào khác được đổi chúng |
| R1–R10 | Phán quyết mặc định của chủ tọa: R1 giữ rail, merge ở owner; R2 "mở" là mở với owner và người của công ty, ra ngoài sau kiểm pháp lý có người ký, tập đánh giá niêm là ngoại lệ thứ hai cạnh credential; R3 lặp tác vụ ghi lại, phát lại, lỗi gieo vào việc lịch sử là quan sát, còn nhánh ngẫu nhiên, holdout, A/B trên khách là thí nghiệm; R4 luật thắng; R5 phân lớp hiệu ứng, hoãn-có-huỷ mặc định tắt; R6 định vị là tổ hợp, không claim tính mới; R7 Remit là tham chiếu, lỗ là việc xây; R8 số theo kiểm toán; R9 ROI không tính được, tài chính bên áp dụng đọc chi phí; R10 ranh giới do bên áp dụng chọn, bên ngoài là nhóm so sánh |
| H0, H1, H2 | Hiệu ứng đảo ngược được trong nền tảng; bù trừ được; không đảo ngược (mục 3.8) |
| pass^k, n_eff, ASR, confused deputy | Xác suất một tác vụ đạt cả k lần lặp; số phiếu judge thật sự độc lập trong một panel; tỷ lệ tấn công thành công; agent dùng quyền hợp lệ của nó để làm việc kẻ tấn công muốn |
| Tập niêm | Tập neo do người gán nhãn, lát niêm của bộ hồi quy, vị trí canary, ngưỡng, nhãn lỗi gieo: owner thấy, agent bị đo và công chúng không thấy |

## 1. Bản này khẳng định gì và không khẳng định gì

**Luận điểm có chủ.** "Kiểm chứng là điểm nghẽn, năng lực sinh thì không" đã được nêu: verifier's law (Wei 2025), generation-verification loop (Karpathy 2025)
[C06 §0, MEDIUM], contract-first decomposition: giao việc chỉ khi kết quả "has precise verification", nếu không thì phân rã tiếp (Tomašev, Franklin, Osindero,
2602.11865, HIGH, [BT]). Bản này áp ý đó vào cấp công ty và đo chính bộ kiểm chứng. **Phân loại** [C06 §2, MEDIUM]: không thành phần nào là phát minh; đã
công bố: checker xác định, hàng đợi ngoại lệ, triển khai một lần; tổ hợp: phân rã objective có review kiểm chứng, ngữ cảnh chung có kiểu, đo độc lập judge,
scorecard vận hành công khai. Prior art gần nhất cho cả hệ là Paperclip (2026, "open-source orchestration for zero-human companies", HIGH[C06] cho README),
vốn có trần ngân sách và board approval; bản này bỏ cả hai và thêm checker xác định, đo độc lập judge. Tổ hợp đó chưa được chứng minh có ích.

**Lựa chọn lệch chuẩn, nói thẳng.** Mọi khung quản trị mà phản biện prior art tìm thấy (35 lượt tìm; coverage, không phải chứng minh [C06 §8]) khuyên giữ giới
hạn: IMDA Singapore (01/2026) "placing limits on agents' powers", "checkpoints at which human approval is required" (HIGH[C06]); Shavit (OpenAI), Google, NIST
NCCoE, OWASP (MEDIUM). Bản này không tự đặt thêm giới hạn phạm vi hay cổng duyệt lên sáng kiến; gate từ chối chỉ với `{NoGrant, Stopped, Law, Rail}`, trong đó `NoGrant` là Least grant của ADR-0018, còn mở (quyết định 3).
Chỗ còn phải chờ do luật hay rail buộc: sổ mục đích cho dữ liệu cá nhân (I17), đọc cách ly không có công cụ hành động (SC1), merge của owner (R1), chữ ký luật
đòi (I12, D11, D12); theo phép thử ở mục 5.2 đó **là** giới hạn, được nhận có tên. Ba cổng trên hiện thực tham chiếu (trần ADR-0001, Least grant ADR-0018, xét
trước ADR-0013) là quyết định 2–4. Không có số đo nào ủng hộ lựa chọn này: nó là **rủi ro, không phải bằng chứng**, và bị bác nếu sự cố H2 đầu tiên qua một
connection hợp lệ là thứ một giới hạn phạm vi đã chặn, hoặc nếu luật sư trả lời câu hỏi 1 (mục 4.3) rằng phải có kiểm trước.

**Câu đóng góp** (theo [C06 §5], MEDIUM): một chuỗi dữ liệu thực địa, n = 1 tổ chức, về (a) tỷ lệ bắt lỗi của checker và auditor; (b) độ độc lập thực của panel
judge trên dữ liệu kinh doanh (ρ, n_eff trên tập neo); (c) tỷ lệ lỗi thoát 30 ngày theo lớp việc, ở cấu hình mà khảo sát 86 hệ đang chạy thật hoặc thử nghiệm
không phủ (68% của 60 hệ trả lời chạy tối đa 10 bước trước khi người can thiệp; 2512.04123, HIGH, [BT]). Nó **không** gồm bằng chứng nhân quả về lợi nhuận, và
chỉ có giá trị nếu ghi thật, ≥ 12 tháng, kể cả thất bại. Remit là mã đóng và chưa có khách (`remit-design.md` K11, mục 10): dữ liệu này là của owner và đọc trong
công ty; công bố ra ngoài là quyết định riêng khi phân phối rời HOLD, qua bốn kiểm ở mục 6.5. **Mức cao nhất được nói hôm nay** [C06 §4]: một phương pháp vận hành có
thể bác bỏ được cho công ty giao phần lớn việc cho agent, kèm dữ liệu vận hành thật của chính owner, kể cả thất bại. Muốn nói "có lãi" cần báo cáo tài chính kiểm toán độc lập
≥ 12 tháng với đơn vị so sánh bên ngoài; "tổng quát" cần ≥ 3 lớp việc, một lớp không phải code; "an toàn" cần sự cố trên 1.000 kết quả đã xác minh trong ≥ 12
tháng; "thước đo chung" cần ≥ 2 tổ chức độc lập công bố M1–M7 và một bên tính lại khớp.

## 2. Bài toán, luận điểm và ROI cho doanh nghiệp lớn

### 2.1 Bài toán

Doanh nghiệp đã mua AI; điểm nghẽn dời sang người duyệt (MEDIUM, suy ra từ các nguồn dưới):
- Một công ty phần mềm, panel 802 lập trình viên (ước lượng trên 564 người có ≥ 3 tháng hoạt động), 196.212 PR: thông lượng tăng khoảng 12% khi từng
  người bắt đầu dùng AI, tăng tiếp, đạt 2× "only after roughly nine months' usage", mục tiêu đạt ở tháng thứ mười; "load per reviewer doubled"
  (2607.01904, HIGH, [BT]). AI **có** tăng thông lượng; chỉ vế chi phí review ủng hộ luận điểm.
- Faros (10.000+ dev): thời gian review +91%, không thấy tương quan cấp công ty [C05 §4, MEDIUM, vendor]. Remit: 52 MR chờ owner (ADR-0002:40, [BT]; n = 1).
- Gần 9.000 lãnh đạo ở 7 nước báo năng suất lao động +0,2%/năm do AI trong 3 năm, kỳ vọng +0,6% (NBER w34836, HIGH, tự báo cáo, [BT]).

### 2.2 Vì sao bây giờ

1. **Năng lực tăng nhanh hơn độ tin cậy** (E03, E04). Mỗi điểm accuracy tăng thêm chỉ đi kèm 0,18 điểm reliability ở GAIA và 0,33 ở τ-bench (2602.16666
   Hình 1, HIGH, [BT]). Hệ quả (MEDIUM): chờ model tốt hơn không tự giải bài toán tin cậy; phần đó là việc của hệ thống bao quanh model.
2. **Giá token ở năng lực cố định giảm 9–900 lần/năm, ở biên frontier thì không**; chi phí mỗi giờ làm việc của agent gần trần năng lực tăng theo độ dài
   tác vụ [D §8; Epoch; Ord, MEDIUM]. Năng lực sinh là thứ mua được.
3. **Luật đã có hiệu lực.** Luật AI Việt Nam hiệu lực 01/03/2026 và chỉ cho chuyển tiếp với hệ đã chạy trước ngày đó (Đ.35(1)); hệ sinh nội dung đưa ra
   sau 02/08/2026 phải đánh dấu máy đọc ngay (AI Act Đ.50(2); hạn 02/12/2026 chỉ dành cho hệ có trước) [C04 K3, K9, HIGH[C04]].

### 2.3 Luận điểm, dạng bác bỏ được

**Đề xuất (giả thuyết):** ở lớp việc có checker xác định, chi phí trọn gói mỗi đơn vị kết quả đứng vững 30 ngày, ở tháng 9–12 sau ngày 0, thấp hơn cùng lớp
việc ở các đơn vị kinh doanh ngoài ranh giới cùng kỳ (R10). Nếu không, luận điểm **không được ủng hộ** ở lớp việc đó. Không có nhân quả về lợi nhuận nào
ngoài phép so sánh này.

### 2.4 Mô hình giá trị và ROI

Nguồn lợi tức: thông lượng trên việc kiểm chứng được (2607.01904; agent đơn tự làm xong khoảng 30% tác vụ công ty mô phỏng, 2412.14161, HIGH, [BT]); chu
kỳ phản hồi ngắn hơn, lỗi âm thầm bị bắt sớm hơn (chỉ với H0), tài sản kiểm chứng tái dùng (ba nguồn sau: LOW, chưa đo). Không có lợi tức nào từ "cắt 80% chi
phí review/QA" [D §8, P §6]. Theo kỳ `t`, lớp việc `k` (dựng theo [C05 §1.1]; không trần chi phí):

```
Lợi ích(t) = Σ_k N_k · s_k · a_k · c_k · H_k · w     (+ ΔDoanh thu: chỉ ước được bằng so sánh với đơn vị ngoài ranh giới)
Chi phí(t) = S(t) + Σ_k N_k · r_k · w + D(t) + E[L](t) + G(t) + F0/H + F(t)
Hoà vốn một đơn vị kết quả:  s · a · c · H · w ≥ c_đv + r · w,   tức  H* = (c_đv + r·w) / (s · a · c · w)
```

| Biến | Nghĩa | Tình trạng nguồn |
|---|---|---|
| `S(t)`, `N_k` | Tổng chi cho agent, kể cả chain bỏ dở; số đơn vị kết quả agent tự nhận | Một mẫu dogfooding (dưới); `N_k` quan sát, không bị chặn |
| `s_k` | Tỷ lệ đơn vị kết quả có cầu thật (một người lẽ ra sẽ làm việc đó) | **Không có nguồn**; 83% chi đo ở Remit là việc nó tự giao (ADR-0002:38–39, [BT]) |
| `a_k` | Tỷ lệ đứng vững 30 ngày (không mở lại, hoàn tác, khiếu nại) | **Không có nguồn** cho hệ này |
| `c_k` | Tỷ lệ giờ giải phóng thành tiền | **Không có nguồn**; ServiceNow tái đầu tư $100M tiết kiệm headcount, không vào biên lợi nhuận [C05 §1.1, MEDIUM] |
| `H_k`, `w` | Giờ người tương đương; đơn giá giờ có gánh chi phí | `H_k` không có nguồn; `w` khoảng $120/giờ (Ord, "best humans", MEDIUM) |
| `r_k`; `R` | Giờ người duyệt mỗi đơn vị; năng lực review (đơn vị duyệt được mỗi kỳ) | Chỉ có hướng: tải reviewer gấp đôi (2607.01904) |
| `D(t)` | Nợ kỹ thuật quy ra tiền | Chỉ có hướng: +30,26% cảnh báo tĩnh, +41,64% độ phức tạp, kéo dài (2511.04427, HIGH, [BT]) |
| `E[L]` | Tổn thất kỳ vọng sau phần bảo hiểm trả | Không có tần suất; đuôi dày (Knight Capital lỗ hơn 460 triệu USD trong khoảng 45 phút; SEC qua [E §5], MEDIUM); bảo hiểm đang loại trừ AI |
| `G(t)`, `F0`, `H`, `F(t)` | Tuân thủ và quản trị; dựng ngày 0, khấu hao, sửa trong lúc chạy | Không có số; `F0` chưa có ước lượng nào |

**Mẫu đo được của Remit (một mẫu, dogfooding).** $1.126,07 trong 3,5 ngày, khoảng 114 chain, 8 MR merge, $141 trọn gói mỗi MR, $8,25 chi riêng mỗi
chain đã merge (ADR-0002:25–26, HIGH, [BT]); 8 × $8,25 = $66 = 5,9% tổng chi. Giới hạn: một ngày đo có lẽ chỉ nửa ngày; 83% ($933) là việc hệ tự giao
qua Scout, nay đã bỏ (ADR-0023); ADR-0002 bị ADR-0010 thay; cơ sở chi phí có thể là giá tương đương API, không phải tiền mặt thuê bao (ADR-0002:58, LOW). n = 8.

**Độ nhạy (minh hoạ, không phải ước tính).** `c_đv` = $141; `r`, `s`, `a` là giả định của [C05 §1.2]; bảng giả định **`c = 1`** (mọi giờ giải phóng thành
tiền), giả định lạc quan nhất: với `c = 0,5` mọi `H*` gấp đôi. Dòng cuối chỉ đúng nếu cả $141 là token, điều chưa biết.

| `w` | `r` | `s` | `a` | `H*` (giờ người tương đương để hoà vốn, `c = 1`) |
|---|---|---|---|---|
| $120 | 0 | 1 | 1 | 1,18 |
| $120 | 0,5 h | 1 | 1 | 1,68 |
| $64 (lương BLS chưa gánh, MEDIUM) | 0,5 h | 1 | 1 | 2,70 |
| $120 | 0,5 h | 0,5 | 0,9 | 3,72 |
| $120 | 0,5 h | 0,17 | 0,9 | 10,9 |
| $120, giá token giảm 10 lần (nếu $141 toàn là token) | 0,5 h | 1 | 1 | 0,62 |

**Kết luận (HIGH cho cấu trúc, LOW cho mọi con số): ROI hôm nay không tính được cho lớp việc nào.** Dấu của nó do `s`, `a`, `c` và `r` quyết định, không do
giá token; khi việc chờ duyệt vượt năng lực review `R`, chi phí mỗi đơn vị chấp nhận bằng `S/R` và tăng theo chi tiêu không trần [C05 §1.3]: vì vậy trần
hàng đợi (quyết định 2) là câu hỏi kinh tế. Các đầu vào chưa có nguồn là U10 (mục 10).

**Ai đọc chi phí (R9).** ADR-0010 tự ghi điều kiện xét lại "runs for someone other than its owner" (ADR-0010:43, [BT]): tài chính bên áp dụng đọc hoá đơn theo
lớp việc từ ngày 0, **ngoài hệ agent**, mở với người của công ty, không vào thưởng (chỉ số vào thưởng bị tối ưu lệch hoặc bị né, 2210.10760; 2503.11926).
**Cửa sổ đọc:** lợi ích tốc độ "dissipate after two months" (2511.04427, [BT]), thông lượng doanh nghiệp đạt 2× sau khoảng chín tháng (2607.01904): đọc nợ từ
tuần 8–12; không kết luận về lợi tức trước tháng 9–12 (MEDIUM).

### 2.5 Định vị so với lựa chọn khác

| Lựa chọn | Có gì | Thiếu gì | Khi nó tốt hơn |
|---|---|---|---|
| Không làm gì | Không tốn `F0`, không rủi ro mới | Không tích luỹ năng lực kiểm chứng | Việc phần lớn không kiểm được bằng máy |
| Copilot theo người (M365 Copilot, $30/người/tháng, MEDIUM) | Giá biết trước, ít đổi tổ chức | Chi phí dồn vào review (2607.01904) | Việc chủ yếu cần phán đoán người |
| Nền tảng agent doanh nghiệp (Agentforce, ServiceNow, Copilot Studio, OpenAI Frontier, Bedrock AgentCore) | Hợp đồng, định danh, lưu trú dữ liệu, chứng chỉ, niêm credential có SLA [C05 §4, MEDIUM] | Không đo pass^k, false-pass, lỗi thoát 30 ngày | Gần như mọi tiêu chí mua sắm hôm nay |
| Agent code tự chủ (Devin, Cursor) | PR tự động; Devin tự báo 67% PR merge năm 2025 (MEDIUM, vendor) | Người vẫn là lưới chặn nơi "outcomes aren't straightforwardly verifiable" | Chỉ có code, đội review còn dư |

**R6, định vị:** Remit là sản phẩm mã đóng; chưa có khách và phân phối ra ngoài HOLD (`remit-design.md` K11, mục 10). Phương pháp đo và kiểm chứng ở bản này là
cơ sở của thiết kế và chạy **trên** nền tảng agent bên áp dụng đã mua, Remit kể cả [C05 §4]. Khi phân phối rời HOLD, câu với thị trường [C05 §6] là: *"Các công ty đã dùng AI để sinh việc gặp cùng một điểm nghẽn: kiểm chứng. Bản này là một cách trả lời, có thiết kế đo được và
phương pháp nói rõ. Trong các nguồn đã tra, chưa có công ty nào, kể cả chúng tôi, chứng minh cách trả lời này có lợi nhuận."* Khoảng trống SR 26-2 để lại
(agentic AI ngoài quản trị rủi ro mô hình ở ngân hàng Mỹ) là chỗ phương pháp có thể được dùng [C05 §2, MEDIUM].

### 2.6 Đọc luận điểm trong vận hành

So sánh chính là **trong và ngoài ranh giới cùng kỳ** (R10); không có đơn vị so sánh thì ghi "tương quan theo thời gian". Luận điểm **không được ủng hộ** ở
một lớp việc nếu, so với đơn vị ngoài ranh giới: **S1** chi phí trọn gói mỗi đơn vị kết quả đứng vững (M6) không thấp hơn ở tháng 9–12; **S2** giờ người mỗi
đơn vị (M5) không giảm giữa tháng 1 và 6; **S3** pass^k (M1) và tỷ lệ làm lại 30 ngày không tốt lên sau sáu tháng sửa; **S4** nợ (M4) còn tăng ở tháng 3;
**S5** tỷ lệ bắt lỗi gieo (W5) không hơn đoán theo tỷ lệ gốc; **S6** giữ chân, khiếu nại, leo thang (D10) xấu đi. **S7**: một lời agent bị khách hay cơ quan
coi là cam kết của công ty là một sự cố C1 (mục 6.3), sửa với khách và xem lại brief. **S8**: thời gian hiệu chỉnh lại panel sau khi judge đổi phiên bản (W6)
tăng dần là dấu tài sản kiểm chứng đang mục. Đọc S1 và S6 cùng S3, vì chỉ số ngắn hạn có thể tốt lên khi chất lượng xấu đi: một lỗi giảm chất lượng ở Bing
làm truy vấn mỗi người tăng hơn 10% và doanh thu mỗi người hơn 30% (Kohavi 2012, HIGH, [BT]).

**Mức đọc là Đề xuất (giả thuyết), không kích hoạt hành động tự động.** Trước ngày 0, nhà tài trợ của bên áp dụng ghi hành động sẽ xét ở mỗi mốc đọc (tiếp
tục, thu hẹp ranh giới, dừng một lớp việc, dừng cả ranh giới) khi S1–S6 không được ủng hộ; báo cáo hội đồng ở mốc đó ghi hành động đã chọn và lý do. **Thiên
lệch chọn:** quy tắc chọn ranh giới (mục 6.1) làm nhóm trong và ngoài khác nhau từ đầu; so sánh chỉ đọc được nếu xu hướng hai nhóm trước ngày 0 song song
(kiểm trên dữ liệu đã có), và mọi kết quả ghi giới hạn đó (MEDIUM).

## 3. Thiết kế

Toàn bộ thiết kế ở mục 3 và 6 là **Đề xuất (giả thuyết)** của hội đồng; cột "Cơ sở" là nguồn của ý, không phải bằng chứng rằng nó đủ.

**Hệ quả nói thẳng.** Không giới hạn sáng kiến thì tổn thất của một lỗi chỉ phụ thuộc thời gian phát hiện, thời gian dừng và độ đầy đủ của phục hồi. Với
hiệu ứng đảo ngược được, ba đại lượng đó cho một cận đo được; với hiệu ứng không đảo ngược, phục hồi chỉ giới hạn lần sau [C03 §4]. Tổn thất không cận ở
lớp H2 vì vậy **đi ra từ "không giới hạn" cộng tính không đảo ngược**, không từ một quyết định nhận rủi ro nào; nó trái lập trường "take a risk when its loss
is bounded and recovery is cheap" (`AGENTS.md:21–22`). Owner phải chọn một trong ba: sửa `AGENTS.md:21` cho lớp H2, bật hoãn-có-huỷ, hoặc giữ hiệu ứng
H2 ngoài ranh giới (quyết định 11; mặc định của chủ tọa: giữ H2 ngoài ranh giới đầu tiên). "We do it all at once" (`AGENTS.md:23–25`) nói về một thay đổi mã
làm trọn trong một lần, dựa vào phục hồi xây trước; với H2 không có phục hồi nào để xây trước.

**Nhánh thân bài giả định** (mặc định của chủ tọa, mục 11): giữ trần ADR-0001 (2), thay Least grant bằng sổ mục đích (3), xét thước đo không chặn (4), bật SC1
(8), hoãn-có-huỷ tắt (9), H2 ngoài ranh giới đầu tiên (11). Mục 7 mô tả hiện thực tham chiếu như nó đang chạy.

### 3.1 Invariant kiểm được

| ID | Invariant | Phép thử bác bỏ | Cơ sở |
|---|---|---|---|
| I1 | Mọi tác dụng ra ngoài đi qua một đường ra của nền tảng; gate chỉ từ chối với mã trong tập đóng `{NoGrant, Stopped, Law, Rail}`; đích là bản ghi nền tảng (I4) thuộc `Rail`; `NoGrant` là Least grant của ADR-0018, còn mở (quyết định 3) | Diễn tập đo lưu lượng mạng của host: byte ra không có bản ghi gate là vi phạm; từ chối ngoài tập đóng là giới hạn ẩn | Reference monitor; hiện thực tham chiếu **chưa** đạt (mục 7); code hôm nay có `no_grant` và `rail` (`internal/grant/grant.go:69-70`), `Stopped` và `Law` đến khi đơn vị dừng (U2) và luật (U9) của `remit-design.md` được làm |
| I2 | Agent không bao giờ giữ credential | Canary credential và quét **mọi** credential trên host, không chỉ giá trị trong kho | PocketOS [D §6]; ADR-0018:112–114 |
| I3 | Mọi hành động hiện ở màn hình của agent đó cho owner trong thời hạn công bố; dữ liệu cá nhân theo phân quyền mục đích (I17) | Hành động có trong ledger mà không có trên giao diện | `AGENTS.md:46`; BVDLCN Đ.30(3) |
| I4 | Log, trace, ledger do nền tảng ghi, chỉ thêm, nơi phiên không ghi được: `source_calls` nằm trong DB mà phiên không đọc, log tệp nằm trong `Hide`. Chuỗi băm là mục hoãn có tên (`remit-design.md` K4, mục 11). Owner bỏ qua sao lưu và khôi phục (K1): snapshot hằng ngày nằm ngoài tầm phiên, việc đã làm nằm trong Git (K6) | Thử ghi, xoá từ mọi công cụ: bị từ chối; kiểm chuỗi băm hằng ngày khi chuỗi băm có | PocketOS; o1 giữ lời dối > 85% câu hỏi tiếp (2412.04984, HIGH, [BT]) |
| I5 | Dừng có hiệu lực trong thời hạn công bố ở mọi mức (ranh giới, tenant, cell, agent, nhà cung cấp model), cả cây tiến trình | Dừng giữa lời gọi; đo ở mạng host tới gói cuối; đếm tiến trình con còn sống | 2509.14260; ADR-0021:63 |
| I6 | Verdict có ít nhất một `evidence_ref` do nền tảng cấp | Verdict chỉ có văn bản bị schema từ chối | 2609.14767; 2603.16244 |
| I7 | Finding mang `reason_code` trong tập đóng | Tỷ lệ `OTHER` được báo | 33,1% của 353 PR bị từ chối (mẫu 717 PR đọc tay) không có lý do quan sát được (2605.22534, HIGH, [BT]) |
| I8 | Tác giả trả lời một finding một lần; không vòng thương lượng | Câu trả lời thứ hai bị từ chối | Nhiều vòng review: +62% báo nhầm, 8,47 so với 5,23 (2603.16244, HIGH, một tác giả, [BT]) |
| I9 | Người kiểm và tác giả là hai pipeline (khác model hoặc họ model, khác prompt, khác người viết); độ độc lập là số đo | Panel thiếu ρ và n_eff trên tập neo được tính là một phiếu | 2605.29800, 2609.22512 |
| I10 | Số khách dựa vào do mã có phiên bản tính; mã và cấu hình của nó chỉ đổi qua merge | Mỗi con số gửi khách truy được về một hàm và một băm cấu hình | Model tốt nhất 82,4%, khoảng 1 câu sai trên 6 (2603.07316, HIGH, [BT]); SOX §404 |
| I11 | Mục tiêu agent tự đặt không vào hàm thưởng hay xếp hạng | Kiểu báo cáo kết quả không có trường mục tiêu tự đặt | Gao 2210.10760; Apollo |
| I12 | Mọi thay đổi prompt, playbook, harness, bộ nhớ chung, model, judge, checker là một release có băm và trường "đổi mức rủi ro pháp lý: có/không" do agent ra release điền, auditor kiểm mẫu; "không" ra ngay; "có" chờ chữ ký của người có tên (Luật AI Đ.10(2)) | Trace có băm ngoài sổ release; release "có" chạy mà không có chữ ký | Hậu kiểm Anthropic [P §4]; Luật AI Đ.10(2) |
| I13 | Ngữ cảnh chung đổi qua ghi có kiểu, theo mục, có phiên bản | API không có thao tác nhận cả tài liệu | ACE 2510.04618 |
| I14 | Mọi tín hiệu phát hiện có người nhận, mức độ, trạng thái | Tín hiệu không người nhận | Knight: 97 email không thiết kế như cảnh báo [E §5] |
| I15 | Mọi lệnh đổi trạng thái mang khoá idempotency | Phát lại ledger hai lần, so trạng thái | `AGENTS.md`, Idempotent by default |
| I16 | Mã của gate, stop, ledger chỉ đổi qua merge; binary đang chạy dựng từ commit đã merge | So băm binary với bản dựng lại | R1; ADR-0012 |
| I17 | Agent đọc dữ liệu cá nhân chỉ khi cặp (role, mục đích) có trong sổ mục đích do người có tên ký; ngoài sổ thì gate trả `Law` cho tới khi người đó thêm mục đích | `Resolve` trả `Law` cho mọi cặp ngoài sổ | BVDLCN Đ.3(2), 9(4)(a), 30(3); GDPR Đ.28(3)(a) |
| I18 | Mọi tin nhắn ra ngoài tới người mang công bố "đây là AI" do gate gắn; nội dung sinh có dấu máy đọc | Quét mẫu đầu ra ở gate | AI Act Đ.50(1),(2); Luật AI Đ.11 |
| I19 | Mỗi tham số hành động (người nhận, số tiền, URL) mang nguồn gốc; tham số từ nội dung ngoài hiện trên bản ghi và màn hình | Bản ghi thiếu nguồn gốc tham số | Đổi tham số là dạng tấn công MCP hiệu quả nhất, ASR trung bình 46,7% (2508.14925, HIGH, [BT]) |

**Chữ ký.** Mọi chữ ký trong bản này (sổ mục đích, I12 "có", hồ sơ D11, công bố D12, tuyên bố rủi ro còn lại) là một tin nhắn đã xác thực của người đó với
Assistant, ghi ai ký (`AGENTS.md:50–52`), không phải biểu mẫu (`AGENTS.md:39–41`); thứ người gõ vào một trường chỉ là credential (`AGENTS.md:52–54`).

### 3.2 Mặt phẳng và vai trò

Owner nói bằng lời thường (đã xác thực) với Assistant; Assistant chuyển cho Chair; Chair viết hợp đồng objective cho các role agent; owner nhìn màn hình từng
agent. Dưới đó là **control plane** bằng mã xác định (một đường ra và gate, stop, ledger, merge, F3) và bốn mặt phẳng: **agent** (planner, đọc
cách ly), **context** (mục có kiểu), **verification** (checker, auditor, tập niêm), **operating** (trace, release). Control plane không đánh giá chất lượng
việc: nó gắn credential, ghi, dừng khi owner bảo, ép luật về **luồng dữ liệu**, giữ rail. Tính hợp pháp của **nội dung** (một phát ngôn có phỉ báng hay
lừa dối không) không quyết được bằng hàm thuần; đó là rủi ro còn lại [C03 §3.4]. **Lớp hiệu ứng do nền tảng gán**, từ bảng (connection, thao tác) → H0, H1,
H2, không do agent tự khai; thao tác chưa có trong bảng được coi là H2 (**Đề xuất (giả thuyết)**).

| Vai | Làm gì | Cơ sở |
|---|---|---|
| Owner, người chịu trách nhiệm ranh giới | Nói objective; kéo rail; ký qua Assistant; đọc ngoại lệ; dừng tay. Một người giữ mọi vai này là điểm đơn lẻ (RK7) | Người kiểm tra kém khi đối kháng và khối lượng lớn [R §2.1] |
| Assistant | Làm theo lời owner; rail nó kéo trỏ tới mã tin nhắn đã xác thực của owner, hiện nguyên văn | `AGENTS.md:50–52`; chống A25 [C03] |
| Chair | Giữ hợp đồng objective, kế hoạch, claim; nhận finding | Task ledger của Magentic-One (2411.04468) |
| Role agent | Đề xuất OKR của role, làm, sửa theo finding | MetaGPT, ablation trên hai tác vụ: 1 → 4 role, số lần người sửa 10 → 2,5 (2308.00352 Bảng 3, HIGH, [BT]) |
| Đọc cách ly | Đọc nội dung ngoài, trả giá trị có kiểu, không có công cụ hành động | CaMeL 2503.18813 (SC1) |
| Auditor | Kiểm độc lập bằng nguồn thô; không thuộc đội; không ghi được thứ nó kiểm | Goal judge ngoài đội (`docs/design.md`; ADR-0013) |

Số role không bị giới hạn bằng mã. Thông tin cho brief của Chair (HIGH, [BT]): kiến trúc ngang hàng đạt 0,477 so với 0,466 của agent đơn nhưng 23,9 so với
67,7 thành công trên 1.000 token (2512.08296 Bảng 5); từ +80,8% trên suy luận tài chính phân rã được (tập trung) đến −70,0% trên lập kế hoạch tuần tự; SWE-bench
Verified giảm nhẹ ở mọi kiến trúc nhiều agent (−15% đến −2%). **Đề xuất (giả thuyết):** ít agent cho code, nhiều agent cho việc phân rã được.

### 3.3 Giao thức objective có review kiểm chứng

1. **Hợp đồng objective.** Chair viết từ lời owner: khách hàng là ai và họ nhận gì (ADR-0016), kết quả, north-star, guardrail, surrogate, chân trời, phần `unknown`.
2. **Đề xuất của role.** OKR, kế hoạch, mỗi key result một `verification_method` máy chạy được. Brief của Chair theo contract-first: việc chưa kiểm được thì
   phân rã tiếp. Đây là phán đoán trong brief, không là cổng: key result vẫn chạy khi `unverifiable`, và tỷ lệ đó là tín hiệu W3.
3. **Kiểm chứng không chặn.** Checker chạy khi đề xuất được ghi; goal judge xét phép kiểm của mỗi yêu cầu song song với việc (mặc định quyết định 4), và một
   kết quả chỉ được tính "đã xác minh" khi phép kiểm của nó đã qua xét. Finding mang `reason_code` (`UNCOVERED_REQUIREMENT`, `UNVERIFIABLE_KR`, `UNKNOWN_METRIC`,
   `OVERLAP`, `GUARDRAIL_BREACH`, `EVIDENCE_CONTRADICTS`, `PROCESS_CONSTRAINT`, `DUPLICATE`, `OTHER(text)`); `PROCESS_CONSTRAINT` tách riêng vì 31,2% của 353 PR
   bị từ chối là do quy trình (2605.22534, [BT]). Tác giả trả lời một lần; chưa giải quyết thì vào hàng đợi ngoại lệ. Thay đổi chạm số khách dựa vào chỉ vào
   production qua merge (I10).
4. **Thực thi.** Key result sinh task; task khai claim; chain giữ kết quả tới khi hiệu ứng được goal judge xác nhận (ADR-0013:33–35, [BT]).

**Cho và chống.** Loop-Back (43 cặp sản phẩm, 86 lần chạy, một loại việc): quyền gửi lại có hại khi người duyệt chỉ góp ý, có ích "when the Manager can verify
the work" (2609.14767, HIGH số, MEDIUM khái quát, [BT]). Chống: kiến trúc tập trung giữ khuếch đại lỗi ở 4,4× nhờ "validation bottlenecks that intercept errors
before aggregation", so với 17,2× khi độc lập (2512.08296, [BT]); thiết kế không chặn bỏ đúng phần lợi ích đó [C02 P1], nên giá trị còn lại của kiểm chứng là
rút ngắn thời gian tới lúc sửa, chỉ đủ với hiệu ứng đảo ngược được.

### 3.4 Ngữ cảnh chung có kiểu

| Thành phần | Đặc tả | Cơ sở và giới hạn |
|---|---|---|
| Mục có kiểu | JSON schema, `version`, `provenance` (ai ghi, từ đầu vào nào, tin cậy hay không), `DataClasses` do nền tảng gắn theo nguồn | Harness có schema [D §5]; LangGraph |
| Ghi | `Write(section, base_version, patch)`; `base_version` cũ bị từ chối kèm diff (tính đúng, không phải quyền); mục có dữ liệu cá nhân theo I17 | ACE 2510.04618; ETag |
| Claim, phép chiếu | Công bố ai đang làm gì, chồng lấn được báo, không bị từ chối; mỗi agent nhận phạm vi của nó cộng tóm tắt phạm vi khác (nhiều lượt: −39%, 2505.06120, [BT]) | Xung đột 48/115 cặp PR khác agent so với 119/601 cặp cùng agent (2607.04697, [BT]); 27,67% PR agent có xung đột merge (2604.03551, [BT]) |
| Nguồn gốc | Nhãn `untrusted` ở mục và tham số (I19), không lan theo phiên (lan theo phiên làm mọi phiên nhiễm và tín hiệu chết, [C03 A10]) | Gắn nhãn đứng riêng chỉ giảm ASR 5% (2410.07283, HIGH, [BT]) |
| Bộ nhớ dài hạn | Một mục có kiểu; đổi bộ nhớ chung là release | Bộ nhớ tự tiến hoá: tỷ lệ từ chối 99,4% → 54,4% (2509.26354, [BT]) |

Chống: CRDT vẫn để xung đột ngữ nghĩa ("preliminary inspection suggests 5–10%", 2510.18893, một tác giả, MEDIUM); chưa có số đo trên tài liệu kế hoạch (LOW).

### 3.5 Lớp kiểm chứng

Thứ tự khi bất đồng: **checker xác định → auditor độc lập → neo hiệu chỉnh**. Kiểm chứng báo và sinh việc sửa; nó chỉ chặn khi đi qua rail F4 hay F3.
- **Tầng 0, checker xác định** (test, schema, bất biến số học, đối soát nguồn thô, chạy song song mã cũ và mới): thực hành chuẩn, không phải đóng góp. Checker
  cho một role do pipeline khác viết; checker chạm số tài chính đổi qua merge (tách người làm và người kiểm, ICFR [C05 §2]).
- **Tầng 1, auditor:** panel trộn judge frontier với open-weight, vì ba judge frontier khác nhà cung cấp cùng sai gấp 7,7 lần mức độc lập và chỉ bằng 1,41 judge
  độc lập (E18), 9 judge chỉ bằng 2,18 phiếu (E17); gộp có tính phụ thuộc (+3,5 điểm so với đa số, 2604.07650, [BT]); đo κ thay tỷ lệ khớp thô, vốn thổi phồng
  33–41 điểm trên MT-Bench, 21 model (2606.19544, [BT]); auditor đọc artifact như dữ liệu `untrusted`, vì chuỗi hời hợt lừa judge tới 80% báo nhầm (2507.08794, [BT]).
- **Tầng 2, neo hiệu chỉnh:** tập neo do người gán nhãn, giữ riêng trước khi giám sát bắt đầu, niêm (R2); nó tách được judge đổi với hệ đổi (E19), còn đổi
  phiên bản judge âm thầm đổi thước đo (E20). Judge là một release riêng.
- **Tỷ lệ bắt lỗi** của checker và auditor đo bằng lỗi gieo vào tác vụ lịch sử đã ghi, trước ngày 0 và liên tục (quan sát theo R3; quyết định 6). **Quy trình
  cùng kết quả** (M2): 80,1% lần đạt kết quả vi phạm quy trình (E09). **Nhất quán** (M1): pass^k và lần tệ nhất trên K ≥ 5 lần lặp (E10, E31).
- **Lặp trên dữ liệu khách là một mục đích xử lý:** tác vụ ghi lại, bộ hồi quy, tập neo từ dữ liệu khách cần cơ sở pháp lý và nằm trong sổ mục đích (I17).

### 3.6 KPI, thưởng và nhân quả không thí nghiệm

**North-star** (doanh thu, giữ chân) đọc sức khoẻ theo quý, không thưởng; **surrogate** (một loại thay đổi, có chủ, có hạn) đọc tác động loại đó; **guardrail**
để báo, không để tối ưu; mục tiêu **tự đặt** chỉ để đo hiệu chỉnh (I11). Surrogate chỉ tốt trong cùng loại thay đổi: một người tham gia nghiên cứu nêu ví dụ giả định về khai
thác (gửi thông báo mỗi 7 ngày để kéo một tương quan ngắn hạn), và các tác giả kết luận "there is no substitute for a well-run long-term experiment" (2608.08043,
HIGH, [BT]). Surrogate index ở Netflix: 79% precision, 65% recall so với quyết định dựa trên số đo trực tiếp ở tháng thứ hai (2311.11922, [BT]).

Quyết định không thí nghiệm loại thí nghiệm dài hạn; còn lại phương pháp quan sát, yếu hơn nhưng khác 0 **nếu có đơn vị so sánh**: difference-in-differences
(2607.01904 dùng staggered DiD trên dữ liệu quan sát), synthetic control, CausalImpact [C06 §4, MEDIUM]. Bật cùng lúc bên trong ranh giới xoá so sánh bên
trong và xoá sự so le mà 2607.01904 dựa vào; đơn vị ngoài ranh giới (R10) là nhóm so sánh, với thiên lệch chọn ở mục 2.6. Chiến lược nhận dạng công bố
trước ngày 0 (quyết định 7). Hiệu chỉnh surrogate bằng kết quả quan sát không có bảo đảm nhận dạng của surrogate index (Athey, NBER w26463, cần mẫu ngẫu
nhiên), nên surrogate chỉ dùng như tín hiệu, không như bằng chứng tác động. **Thử thước đo** bằng dữ liệu phát lại có lỗi chủ ý, không trên khách. **Detector
không vào thưởng** (monitor vào reward thì bị né; monitor chuỗi suy luận recall 95%, 2503.11926, [BT]).

### 3.7 Rail, điểm cố định và gate

| Mục | Hiện thực | Mã từ chối |
|---|---|---|
| F1 Dừng tay | `StopState` theo ranh giới, tenant, cell, agent, nhà cung cấp; dừng cả cây tiến trình; đo ở mạng host | `Stopped` |
| F2 Credential kín | Executor (backend, proxy chèn credential, jail) gắn secret; agent giữ tên connection; `Redact` chống rò **vô ý**, không chống mã hoá cố ý | — |
| F3 Luật | `LegalPolicy` ép luật về luồng dữ liệu: I17, I18, nhà cung cấp có DPA, và quyết định mà luật bắt có người được liệt kê theo loại `Action` trong một bảng do luật sư duyệt | `Law` |
| F4 Rail `AGENTS.md:50–54` | Xác thực; pause và stop; merge do owner hoặc Assistant theo tin nhắn đã xác thực, ghi ai làm | `Rail` |
| Bản ghi nền tảng | Log, ledger, snapshot không là đích của công cụ nào (I4) | `Rail` |

```go
type Refusal string // NoGrant | Stopped | Law | Rail (closed set, I1; NoGrant while ADR-0018's least grant stays); Action carries EffectClass (H0|H1|H2, set by the platform),
                    // Params with origin (I19), DataClasses, Purpose, Target, IdempotencyKey, ConfigHash, OwnerMessage when the Assistant pulls a rail
func Resolve(a Action, grants []Grant, s StopState, law LegalPolicy, rails RailSet, records RecordSet) Resolution // pure
func Tripwires(a Action, h Window, cfg SealedThresholds) []Alert                                 // pure; never an input to Resolve
func Disclose(msg Outbound) Outbound                                                             // pure; I18
```

Property test: với mọi `Action` sinh ra, `!Execute` ⇒ mã thuộc tập đóng, mỗi mã có điều kiện riêng (`Stopped` ⇔ phạm vi trong `StopState`; `Rail` ⇔ effect là
rail mà actor không phải owner, hoặc Assistant thiếu `OwnerMessage` đã xác thực, hoặc đích thuộc `RecordSet`; `Law` ⇔ `LegalPolicy` trả vi
phạm; `NoGrant` ⇔ chain không giữ grant nào phủ hành động, khi Least grant còn); ngoài đó là `Execute`. Phép thử lấy mẫu nên chỉ **cho thấy** trên các `Action` đã sinh rằng `Resolve` không chứa giới hạn ẩn, không chứng minh; scope
của connection, rate limit của nhà cung cấp, sandbox cần diễn tập I1 [C06 §3 #14]. `Tripwires` không là đầu vào của `Resolve` (đồ thị phụ thuộc package).

### 3.8 Lớp hiệu ứng (R5)

Quy tắc phân lớp: công ty có tự mình hoàn tác được không, không cần bên kia hợp tác.

| Lớp | Ví dụ | Phục hồi xây trước | Cận tổn thất |
|---|---|---|---|
| **H0** đảo ngược trong nền tảng | Mã trong Git, cấu hình, mục ngữ cảnh, dữ liệu nội bộ có bản lưu ngoài tầm agent (dữ liệu điều phối của Remit chỉ có snapshot hằng ngày và không có mã khôi phục, K1; việc đã làm có trong Git, K6) | Revert theo băm; phát lại ledger; khôi phục; xoá mềm, snapshot trước khi ghi đè [C03 D5] | Có: phát hiện + dừng + khôi phục, mỗi số đo bằng diễn tập |
| **H1** bù trừ được | Con số khách thấy (sửa kèm thông báo), tín dụng chưa chi trả, giá trong catalog | Bản ghi bù trừ; thông báo người bị ảnh hưởng | Có, cộng chi phí bù; niềm tin và quyết định khách đã ra thì không |
| **H2** không đảo ngược | Email, tin nhắn, phát ngôn công khai, tiền đã chuyển ra (kể cả hoàn tiền đã trả, tiền đã trả cho nhà cung cấp model), dữ liệu tới bên thứ ba hay nhà cung cấp, xoá ở hệ thống bên thứ ba, cam kết ràng buộc (Luật GDĐT Đ.14(2)(a); UETA §14) | Không có gì trả lại lần này | **Không có cận**; trái `AGENTS.md:21` (quyết định 11) |

Với H2, các biện pháp không chặn hành động: SC1, I19, SC6 (chỉ báo), và **hoãn-có-huỷ** (SC4): nền tảng giữ hành động H2 một cửa sổ công bố trước ngày 0, F1
trong cửa sổ rút được, không ai phải duyệt; trong bản này chỉ nó cho H2 một cận đo được (cửa sổ cộng thời gian dừng). Theo R5 nó **mặc định tắt**; red team,
giám đốc rủi ro và người đọc lạnh khuyên bật cho một phần H2 (quyết định 9). Lời gọi model mang dữ liệu khách là H2 được phép theo DPA và sổ mục đích, đi qua
SC2, không qua SC4 (**Đề xuất (giả thuyết)**). H2 còn trong ranh giới cần **tuyên bố rủi ro còn lại** đã ký (mục 6.6).

### 3.9 Lớp vận hành

| Thành phần | Đặc tả | Cơ sở |
|---|---|---|
| Trace | Bước, lời gọi công cụ, verdict: phiên, role, `config_hash`, mã model do API trả về, connection, nguồn gốc tham số, kết quả gate; OTel GenAI khi cần | [P §4] MEDIUM |
| Kiểm release | Eval trên tác vụ ghi lại, K ≥ 5 (lựa chọn của hội đồng), so phân phối (trung bình, pass^k, lần tệ nhất, Wilson); báo, không chặn, trừ release chạm số tài chính hoặc mã rail (merge) hay release I12 "có" | Ví dụ tính của Miller (σ² = 1/6, ω² = 1/9, n = 198): K từ 1 lên 10 giảm hiệu ứng nhỏ nhất phát hiện được 13,2% → 7,5% (2411.00640 §5, [BT]) |
| Theo dõi sau release | Phân phối theo `config_hash` trên lưu lượng thật | Hai hậu kiểm Anthropic: ba lỗi hạ tầng 08–09/2025 (16% yêu cầu Sonnet 4 ở giờ tệ nhất ngày 31/8; eval "simply didn't capture the degradation"; thấy qua phản hồi người dùng) và ba thay đổi harness Claude Code 03–04/2026 (HIGH[C01]) |
| Ghim model, phục hồi, tiến độ | Snapshot có ngày, lịch ngừng (Anthropic báo trước tối thiểu 60 ngày, MEDIUM), chạy lại eval khi có thông báo; H0 của mã khôi phục từ Git bằng revert theo băm, dữ liệu điều phối do owner bỏ qua sao lưu (`remit-design.md` K1); không tiến triển sau N vòng thì Chair được báo, owner thấy | [P §4]; [C05 §5 #5]; Vending-Bench: mọi model có lần chạy "derail" [E §4] |

### 3.10 Giao diện của owner

Người canh agent thường thất bại (E21); chỉ chuyển ca đã gắn cờ giúp một phần, không bền trước đối thủ (E22); người duyệt giữ 89% vấn đề LLM nêu (2411.11401,
[BT]); 93% lời nhắc xin quyền của Claude Code được chấp nhận ([R §2.1], MEDIUM; áp sang merge là suy rộng). Vì vậy: (1)
**màn hình từng agent** có mọi hành động, đếm việc chờ owner ở tab của agent đó (`AGENTS.md:46`); (2) **hàng đợi ngoại lệ** là một khung nhìn gom các mục đang
chờ trên màn hình từng agent, máy xếp theo lớp hiệu ứng rồi theo tuổi, gộp trùng; mục tiêu đo được: với mọi lỗi gieo lớp H2, ngoại lệ hiện trong ≤ T phút, T và
người trực công bố trước ngày 0 và niêm (R2); (3) **bằng chứng do nền tảng dựng** (diff, kết quả checker, con số do mã tính, nguồn gốc tham số), tách khỏi lời
agent (Vasconcelos 2212.06823); (4) **kết quả của những gì đã merge**: tỷ lệ merge không bình luận, lỗi thấy sau merge. Ngoài merge (R1), owner đọc kết quả và
cho khẩu vị, không phải lưới chặn của từng kết quả (`AGENTS.md:56`).

### 3.11 Bằng chứng cho và chống

Cho và chống của từng phần tử nằm ở mục của nó và ở bảng 8.1; bảng đầy đủ ở `council/02-design.md` §7, `council/challenge-06-novelty.md` §2. Không có số đo
nào ủng hộ gate bốn mã và "không giới hạn sáng kiến" (LOW rằng tổn thất H2 có cận), hay triển khai một lần (một ca thất bại đã biết: Hershey 1999, MEDIUM).

## 4. Thứ luật buộc quay lại (hệ quả của F3, R4)

**Nguyên tắc.** Khi luật va với "không giới hạn", luật thắng. Agent không bao giờ là người chịu trách nhiệm: người chịu là pháp nhân và người đại diện theo
pháp luật là cá nhân (Luật Doanh nghiệp 2020 Đ.12(1)); Luật AI Đ.4(2): AI "không thay thế thẩm quyền và trách nhiệm của con người". Công ty đưa hệ vào dùng
dưới tên mình là **nhà cung cấp** (Luật AI Đ.3(4)) và dùng trong kinh doanh là **bên triển khai** (Đ.3(5)): cả hai [C04 K11]. Nguồn: [C04] đọc toàn văn từ OJ,
Federal Register hoặc cơ sở dữ liệu luật Việt Nam, chưa đối chiếu Công báo; hội đồng chưa mở lại: **HIGH[C04]** cho chữ, MEDIUM cho áp dụng.

### 4.1 Theo nơi

| Nơi | Luật đòi | Thiết kế mang nó bằng | Nhãn |
|---|---|---|---|
| VN | Luật AI 134/2025: hệ bật sau 01/03/2026 không có chuyển tiếp (Đ.35(1)); agent nói với công chúng là rủi ro trung bình (Đ.9(1)(b); NĐ 142 Đ.9); hồ sơ phân loại và thông báo Bộ KH&CN **trước khi dùng** (Đ.10(1),(3); NĐ 142 Đ.12, 14); phân loại lại khi sửa đổi phát sinh rủi ro mới (Đ.10(2)) | D11 hồ sơ ký; I12 trường "đổi mức rủi ro" | HIGH[C04] chữ |
| VN | Đ.4(2) "kiểm soát và khả năng can thiệp của con người đối với mọi quyết định và hành vi"; Đ.7(4) cấm cản trở giám sát của người | F1 là điều kiện cần; đủ hay không: câu hỏi 1 | HIGH[C04] chữ; MEDIUM hệ quả |
| VN | Đ.11 công bố tương tác AI, đánh dấu máy đọc; Đ.12(2)(a), NĐ 142 Đ.19: sự cố nghiêm trọng báo sơ bộ 72 giờ (khẩn cấp) hoặc 05 ngày làm việc, chính thức 15 ngày, lưu nhật ký; Đ.13(3) đánh giá phù hợp cho rủi ro cao; QĐ 33/2026 sáu lĩnh vực rủi ro cao, mục ngân hàng ghi "trong hoạt động ngân hàng" | I18, D13; C0 ở mục 6.3; QĐ 33 nhiều khả năng không chạm công ty không phải tổ chức tín dụng | HIGH[C04]; QĐ 33 MEDIUM (báo chí) |
| VN | Luật 91/2025 (BVDLCN, hiệu lực 01/01/2026): đồng ý theo từng mục đích (Đ.9(4)(a)); hệ AI "phân quyền truy cập" (Đ.30(3)); hồ sơ đánh giá tác động xử lý và hồ sơ chuyển ra nước ngoài trong 60 ngày (Đ.21, 20); báo vi phạm 72 giờ (Đ.23); chỉ định nhân sự bảo vệ dữ liệu (Đ.33(2)); phạt 5% doanh thu chỉ cho vi phạm chuyển xuyên biên giới (Đ.8(4)), vi phạm khác tối đa 3 tỷ đồng (Đ.8(5)) | I17 sổ mục đích; D14 | HIGH[C04] chữ |
| VN | Luật An ninh mạng 116/2025 Đ.25(3) lưu dữ liệu người dùng tại VN (từ 01/07/2026); Luật GDĐT 20/2023 Đ.14(2)(a) thông điệp tự động là của người khởi tạo; BLLĐ 2019 Đ.42, 44, 47: trao đổi tổ chức đại diện, báo trước 30 ngày cho UBND tỉnh và người lao động, trợ cấp | Cam kết của agent là H2; thay người theo lịch luật, không ở ngày 0 | HIGH[C04] chữ; MEDIUM phạm vi Đ.25(3) |
| EU | AI Act áp cho bên ở nước thứ ba khi đầu ra dùng ở EU (Đ.2(1)(c)); Đ.50(1),(5) báo cho thể nhân (kể cả nhân viên khách B2B) từ lần tương tác đầu, từ 02/08/2026; Đ.50(2) đánh dấu máy đọc (Reg. (EU) 2026/1744, OJ 24/07/2026); phạt tới 15 triệu EUR hoặc 3% (Đ.99(4)) | I18, D13 | HIGH[C04] |
| EU | Annex III 4(b) từ 02/12/2027: AI giao việc, theo dõi, đánh giá người lao động là rủi ro cao; Đ.14(4)(d) người override từng quyết định; Đ.26(7) báo đại diện người lao động trước; Đ.9(8) test trước khi đưa ra thị trường | Quyết định về việc làm của một người có người ký; nếu một chức năng rơi vào rủi ro cao, test trước thắng "không thí nghiệm" | HIGH[C04] chữ; MEDIUM rằng blueprint chạm vào |
| EU | GDPR: processor theo chỉ dẫn (Đ.28(3)(a)); tự đặt mục đích thì thành controller (Đ.28(10)); DPIA trước (Đ.35); báo 72 giờ (Đ.33); quyết định "solely" tự động (Đ.22); SCC vì VN không có adequacy. PLD 2024/2853: sản phẩm sau 09/12/2026, chỉ thể nhân; thiệt hại kinh doanh của sole trader ngoài PLD. 98/59/EC; BetrVG §87(1) Nr. 6 đồng quyết với hệ có khả năng giám sát hiệu suất | I17; DPA với mọi nhà cung cấp model; gói hội đồng lao động | GDPR MEDIUM; PLD, 98/59, BetrVG HIGH[C04] chữ |
| US | EO 14365 không tự preempt luật bang; Colorado SB 26-189 từ 01/01/2027 (ADMT trong 7 lĩnh vực, giải thích trong 30 ngày, human review); FTC §5 và quy tắc review giả 08/2024; UETA §14; WARN 60 ngày khi tới ngưỡng; SOX §404 (15 U.S.C. §7262) cho issuer niêm yết; California BOT Act, Utah, CPPA ADMT (MEDIUM); TCPA (LOW-MEDIUM) | Release kể cả prompt và cấu hình đổi số tài chính đi qua merge có bằng chứng duyệt; không dùng "kết quả đã xác minh" trong marketing | HIGH[C04] chữ; SOX áp dụng MEDIUM |
| UK, SG | UK GDPR Đ.22A–22D từ 05/02/2026; UCTA; TULRCA s.188 (MEDIUM); IMDA Singapore tự nguyện, khách doanh nghiệp sẽ hỏi | Như EU | MEDIUM |
| Ngân hàng | SR 26-2 (17/04/2026) đặt AI sinh và agentic ngoài quản trị rủi ro mô hình; DORA đòi đánh giá rủi ro tập trung; FSB 2013: khung khẩu vị rủi ro gồm "risk limits" | Quyết định 12 | MEDIUM |

### 4.2 Năm điều luật đòi, và chỗ thứ sáu có thể đến

Theo [C04, kết luận], bên áp dụng **có thể** dùng blueprint hợp pháp nếu giữ: (1) giới hạn dữ liệu cá nhân theo mục đích và quyền truy cập, ép ở gate; (2) hồ
sơ, thông báo, công bố ra ngoài và báo cáo sự cố do một cá nhân ký; (3) "đây là AI" bằng mã ở mọi kênh ra ngoài; (4) thay người theo thủ tục và lịch luật lao
động; (5) với issuer niêm yết, merge cho mọi release chạm số liệu tài chính. Câu trả lời cho Đ.4(2) có thể buộc điều thứ sáu (một dạng kiểm soát trước).
"Không giới hạn phạm vi" vì vậy đọc là: **trừ giới hạn dữ liệu theo mục đích mà luật đòi**. Dừng là quyết định của owner; báo cáo sự cố thì không. **Đề xuất
(giả thuyết)**, không phải luật: quyết định nhân sự đọc tín hiệu (D10, M5) sau ít nhất một chu kỳ đọc, không đọc kế hoạch [C04 §4].

### 4.3 Câu hỏi cho luật sư (điều kiện bật chức năng tương ứng, xếp theo hậu quả)

1. **Luật AI Đ.4(2), 14:** nút dừng, log đầy đủ và quyền đảo ngược sau có đáp ứng "kiểm soát và khả năng can thiệp ... đối với mọi quyết định và hành vi" khi không có duyệt trước? Cơ quan có xử lý Đ.4(2) như nghĩa vụ?
2. **Phân loại:** agent hỗ trợ, bán hàng, marketing có tự động là rủi ro trung bình khi đã công bố là AI? Mỗi role một hồ sơ hay một hệ? Sửa brief, đổi model có là "sửa đổi làm phát sinh rủi ro mới"?
3. **Vai trò dữ liệu:** khi agent tự chọn việc, công ty còn là processor (GDPR Đ.28(10))? Mức phân quyền nào thoả BVDLCN Đ.30(3), GDPR Đ.25, 32?
4. **Lao động:** thủ tục BLLĐ Đ.42, 44 cho từng đợt; cơ sở chưa có tổ chức đại diện thì trao đổi với ai; ngưỡng 98/59, WARN, TULRCA; BetrVG §87, §111.
5. **Sự cố:** ngưỡng "thiệt hại đáng kể về tài sản" (NĐ 142 Đ.19(1)(b)) cho một con số sai gửi nhiều khách; thứ tự báo cơ quan VN, EU, khách, postmortem.
6. **Đ.50 AI Act:** dạng công bố đủ cho email, chat B2B; email agent soạn, người ký gửi có thuộc không; có phải chỉ định đại diện EU (GDPR Đ.27)?
7. **SOX:** release prompt, playbook, model chạm số tài chính có thuộc ITGC; bằng chứng duyệt nào được chấp nhận; một owner duyệt mọi merge có đủ tách nhiệm vụ?
8. **Hợp đồng với khách:** giới hạn trách nhiệm cho con số và cam kết của agent ở California, New York, Đức, Anh (UCTA), VN; rút cam kết sai (Luật GDĐT Đ.14(3)); ai gánh khoảng chênh với trần trách nhiệm vendor (phí 12 tháng, [R §1.4], MEDIUM)?
9. **QĐ 33:** toàn văn trên Công báo; mục ngân hàng có chạm công ty tự động thanh toán, hoàn tiền mà không phải tổ chức tín dụng?
10. **Chuyển dữ liệu:** NĐ 356 đòi gì thêm Đ.20; một hồ sơ cho mọi nhà cung cấp LLM hay mỗi bên; Luật An ninh mạng Đ.25(3) có chạm công ty này?
11. **Công bố và chữ ký:** công bố brief, scorecard, postmortem có vi phạm DPA, NDA, tạo thừa nhận trách nhiệm, đi trước nghĩa vụ báo cơ quan; cần đặc quyền luật sư? Chữ ký trên tuyên bố rủi ro còn lại có thành bằng chứng chống người ký (Marchand)?
12. **Bảo hiểm:** hợp đồng hiện có loại trừ AI sinh (Verisk/ISO CG 40 47 từ 01/01/2026, MEDIUM, mẫu chưa ai mở)? Luật AI Đ.14(5) khuyến khích bảo hiểm cho rủi ro cao.

## 5. Bảo mật

### 5.1 Bề mặt tấn công theo ranh giới tin cậy [C03 §2]

| Ranh giới | Kịch bản chính | Số đo | Tầm hại |
|---|---|---|---|
| B1 nội dung ngoài → agent | Email, ticket, web chứa chỉ thị; đổi tham số của hành động hợp lệ | ASR có mục tiêu 47,69% (GPT-4o), 33,86% (Claude 3.5 Sonnet) (2406.13352, [BT]); gián tiếp 27,1% so với trực tiếp 5,7% trên khoảng 1,8 triệu tấn công (2507.20526, [BT]); web khiến hệ nhiều agent dùng GPT-4o chạy mã tuỳ ý 58–90% tuỳ orchestrator, có cấu hình 100% (2503.12188, [BT]) | Lớn, H2 |
| B2 agent ↔ agent, ngữ cảnh chung | Chair bị chiếm qua kết quả sub-agent; injection tự nhân bản; đầu độc bộ nhớ | Orchestrator không chạm nội dung ngoài vẫn bị chiếm (2503.12188) | Lớn |
| B3 agent → công cụ | Mô tả công cụ bị đầu độc từ đầu; agent tạo công cụ có lỗ hổng; rug-pull | ASR cao nhất 72,8%, model mạnh dễ bị hơn (2508.14925, [BT]); tạo hoặc dùng lại công cụ có lỗ hổng > 76%, không nhận ra công cụ độc gần 93% (2509.26354, [BT]) | Lớn |
| B4 agent → production, tiền, khách | Hoàn tiền, email hàng loạt, chuyển tiền chia nhỏ; confused deputy với credential niêm | Tấn công thích ứng vượt 12 phòng thủ tĩnh với tỷ lệ trên 90% cho đa số (2510.09023, [BT]) | Lớn, H2 |
| B5 agent → nhà cung cấp LLM | Dữ liệu cá nhân đi qua kênh của phiên, ngoài `LegalPolicy` | Đường đi có trong mã (ADR-0018:71–72; `sandbox_darwin.go:143`) | Lớn, H2 |
| B6, B7 owner ↔ hệ; auditor ↔ bên sản xuất | Ngập hàng đợi; agent soạn câu hỏi để owner nói "merge"; Assistant kéo rail theo lời bịa; chuỗi lừa judge; chỉ thị cho auditor trong artifact; agent biết vị trí detector | 9/16 phá hoại thành công dù monitor báo đúng (2606.05647); FPR tới 80% (2507.08794); detector vào thưởng bị né (2503.11926) | Lớn; Vừa–Lớn |

**Thiệt hại chính không phải lộ secret** mà là dùng connection hợp lệ cho việc trái phép (confused deputy): F2 không chặn được nó [C03 §3.2]. Model phá cơ chế
tắt tới 97% (KTC 96–98%) dù được dặn, trên hơn 100.000 lượt và 13 model, khi cơ chế nằm trong môi trường của model (2509.14260, HIGH, kịch bản dựng, [BT]); lời
dặn trong prompt không phải rail.

### 5.2 Thay đổi thiết kế

Phép thử [C03 §1]: một biện pháp là **giới hạn** nếu nó khiến hành động agent đã chọn không xảy ra hoặc phải chờ một chủ thể khác; đổi kênh, nhãn, bằng chứng
hay khả năng hoàn tác thì không. Theo phép thử này, rail F4 (merge), các chữ ký luật buộc (I12, I17, D11, D12), đọc cách ly không có công cụ hành động (SC1)
và quy tắc "không chữ ký thì không bật" (quyết định 15) **là** giới hạn; bản này nhận chúng có tên vì F3 và R1 buộc. Với bên áp dụng, cột cần đọc là "chặn
gì, tốn gì".

| # | Thay đổi | Chặn gì; tốn gì | Bằng chứng |
|---|---|---|---|
| SC1 | Tách luồng điều khiển khỏi dữ liệu: planner chỉ thấy đầu vào tin cậy; nội dung ngoài qua đọc cách ly, thành giá trị có kiểu, không thành lệnh | Không chặn hành động agent đã chọn; nội dung ngoài không ra lệnh được; tiện ích giảm | "we install no security policy and all security benefits come from explicit separation"; Gemini 2.5 Pro 300 → 0 trên 949 (2503.18813 §6, HIGH, [BT]); không về 0 ở mọi model; role làm theo lời khách vẫn biến dữ liệu thành lệnh |
| SC2 | Một đường ra của nền tảng cho phiên và chương trình; F1, F3 ép ở đó | Không chặn đích nào; tốn một proxy | Điều kiện để I1, I5 đúng |
| SC3 | Nguồn gốc theo tham số (I19) | Không chặn; ghi, hiện | 2508.14925 |
| SC4 | Hoãn-có-huỷ cho H2 | Không chặn; trễ một cửa sổ; mặc định tắt (R5, quyết định 9) | Tổn thất theo thời gian dừng (Knight) |
| SC5 | Mặc định đảo ngược được: xoá mềm, snapshot, bản ghi bù trừ | Không chặn; tốn lưu trữ | PocketOS, Replit |
| SC6 | Tripwire theo tổng trong cửa sổ (hoàn tiền, email, đích mới) | Không chặn; chỉ báo (quyết định 10) | Chống chia nhỏ [C03 A20] |
| SC7–SC9 | Sổ ghi ngoài tầm phiên (I4; chuỗi băm khi có); canary token (credential giả, bản ghi khách giả) ở vị trí niêm; bằng chứng do nền tảng dựng cho rail | Không chặn; chỉ ghi, báo, hiện | 2412.04984; kẻ thích ứng dùng phản hồi của detector (2510.09023); Vasconcelos |
| SC10 | Rail do Assistant kéo trỏ tới tin nhắn đã xác thực của owner | Chặn rail kéo theo lời bịa; là xác thực | `AGENTS.md:50–52` |
| SC11 | Dừng đo ở mạng host, không ở log của gate | Không chặn; là phép đo | [C03 §3.1] |
| SC12 | Checker đọc mã và mô tả của công cụ, chương trình ở lần đầu đăng ký, rồi băm | Không chặn; chỉ báo | Băm chỉ thấy thay đổi; MCPTox đầu độc từ đầu |

**Không đề xuất vì chặn hành động agent đã chọn:** Rule of Two, danh sách đích được phép, hạn mức chi, cổng duyệt cho email, phần **chính sách** của CaMeL và
FIDES (2505.23643, MEDIUM). Egress không có allowlist; đích mới là tripwire; nhà cung cấp dự phòng chỉ dùng khi có DPA. Bên áp dụng muốn giữ các biện pháp này
vẫn dùng được lớp đo (mục 0); khi đó họ không chạy lập trường gate của bản này.

### 5.3 Rủi ro còn lại

Confused deputy qua injection là một trong các rủi ro lớn nhất ở ngày 0 (MEDIUM, xếp hạng suy luận; [C03 §6] xếp F1 ngoài gate trước, [C02 §4] xếp hàng đợi owner
sập trước). Với SC4 tắt, chỉ SC1 đứng trước một H2, và SC1 không bảo vệ role mà việc chính là làm theo lời khách. Không dựa vào "an toàn nhờ kém" (WASP: hoàn thành mục tiêu kẻ tấn công 0–17%, 2504.18575): model mạnh hơn
dễ bị đầu độc công cụ hơn (2508.14925). Hiện thực tham chiếu chưa thoả I1, I2, I5 (mục 7).

## 6. Ra mắt

### 6.1 Một lần, trong một ranh giới

Bên áp dụng chọn một đơn vị kinh doanh có dữ liệu, kho secret, backup, log, nút dừng và một người chịu trách nhiệm có tên; bên trong mọi thành phần bật cùng lúc,
trừ chức năng còn chờ một việc luật đòi (Khung). Hệ quả của điểm cố định: ranh giới chỉ gồm hệ thống mà **mọi** tác dụng của agent đi qua gate và proxy (F1, F2 chỉ đúng ở đó; [C02 pre-mortem 5]); luật
áp theo nơi khách và pháp nhân ở; đơn vị bên ngoài giữ cách làm cũ và là nhóm so sánh quan sát (R10). **Quy tắc chọn:** đơn vị mà việc bên trong phần lớn kiểm
được bằng máy, và theo mặc định quyết định 11, không có hiệu ứng H2. Thất bại đến sớm ở hành chính, tài chính, khoa học dữ liệu (2412.14161, [BT]), lập kế hoạch
tuần tự (2512.08296) và hỗ trợ khách (2610.25954).

### 6.2 Phải có ở ngày 0 (mỗi mục một phép kiểm; phép kiểm xác minh thứ đó tồn tại và chạy, không duyệt agent; chưa có ước lượng chi phí hay thời gian dựng)

| # | Thành phần | Phép kiểm |
|---|---|---|
| D1 | F1 dừng | Dừng từng mức; ở mạng host không còn byte ra từ tiến trình nào của agent; không còn tiến trình con; gate từ chối mọi lời gọi và mọi lần dùng credential; agent không có lệnh bỏ dừng |
| D2 | F2 credential | Quét mọi bề mặt agent đọc được (tệp, repo, bộ nhớ, `~/.ssh`, `~/.config/glab-cli`, môi trường mọi tiến trình cùng user) tìm mọi credential: 0 kết quả |
| D3 | Snapshot và Git | Snapshot hằng ngày (`store/snapshot.go`) đọc và xoá bằng mọi credential agent dùng: bị từ chối; mỗi kết quả đã merge truy được trong Git trên remote. Khôi phục bằng tay (chép tệp về) làm được; mã `Restore` và diễn tập khôi phục không có vì owner bỏ qua (`remit-design.md` K1) |
| D4 | Log nền tảng | Mỗi hiệu ứng lấy mẫu ở hệ ngoài có dòng trong sổ nền tảng (`source_calls`; chuỗi băm khi có) kèm nguồn gốc từng tham số, mã model thực, phiên bản prompt, playbook, harness, bộ nhớ; sửa bằng danh tính agent: bị từ chối |
| D5 | Quan sát trực tiếp | Mở màn hình từng agent trong trình duyệt: hành động, câu hỏi, lỗi hiện trực tiếp; tab đếm việc chờ owner |
| D6 | Change log, hoàn tác | Sửa một dòng playbook, thấy phiên bản mới, hoàn tác bằng một hành động; trường I12 có/không được điền; release "có" không chạy khi thiếu chữ ký |
| D7 | Bộ hồi quy | Ca từ lỗi thật trong lịch sử và phiên dài nhiều lượt; mỗi ca K ≥ 5 lần; có lát mở và lát niêm |
| D8 | Danh tính model | Mã snapshot có ngày được ghi; mã thực phản hồi khác mã đã chọn thì hiện cho owner |
| D9 | Tool tính số | Mỗi con số khách dựa vào có một tool bằng mã; tỷ lệ con số gửi khách qua tool đó được đo |
| D10 | Nguồn độc lập | Khối lượng liên hệ, leo thang, khiếu nại, CSAT lấy từ hệ agent không ghi được, ở cả trong và ngoài ranh giới |
| D11 | Hồ sơ pháp lý | Mỗi mục "trước khi dùng" ở mục 4.1 có văn bản do người đại diện ký, có ngày |
| D12 | Công bố ra ngoài | Mỗi lần xuất qua bốn kiểm ở mục 6.5, có tên người ký |
| D13 | Thông báo AI | Gate thêm "đây là AI" vào mọi tin nhắn, email, cuộc gọi tới người ngoài từ lần đầu; nội dung sinh có dấu máy đọc |
| D14 | Dữ liệu cá nhân | Sổ mục đích do người ký; gate so lớp dữ liệu với role và mục đích; cặp ngoài sổ nhận `Law` |
| D15 | Canary token | Chạm tới thì báo owner, không dừng gì; vị trí niêm |
| D16 | Công cụ mới | Ghim phiên bản; checker so mô tả với hành vi, tìm chỉ thị trong mô tả; chỉ báo |
| D17 | Tách luồng SC1 (mặc định *bật* của quyết định 8), tripwire có người nhận, hàng đợi ngoại lệ | Chair và mọi role có công cụ H2 chạy qua đọc cách ly; một chỉ thị gieo trong nội dung ngoài không thành lệnh; mọi tín hiệu có người nhận (I14) |
| D18 | Đo bộ kiểm chứng | Tỷ lệ bắt của checker, auditor và thời gian phát hiện theo lớp hiệu ứng, đo trên tác vụ ghi lại; tập niêm; chiến lược nhận dạng và hành động dự định ở mỗi mốc đọc (mục 2.6) đã công bố; tuyên bố rủi ro còn lại đã ký |

### 6.3 Quan sát và sửa trong lúc chạy

**Sai được nhận ra từ đâu**, xếp theo độ tin (MEDIUM, suy luận): kiểm xác định trên log (bước playbook vắng, số không qua tool, mã model lệch) → bộ hồi quy chạy
mỗi thay đổi và liên tục trên phiên bản đang dùng → nguồn độc lập (D10) và canary (D15) → phản hồi của khách, chuyển thẳng tới agent phụ trách → auditor và judge
→ lời agent tự báo không phải tín hiệu "xong". **Sửa tại chỗ:**

| Loại | Ví dụ | Ai làm | Nghĩa vụ luật (chạy dù owner có dừng hay không) |
|---|---|---|---|
| C0 | Mất dữ liệu, cam kết sai, secret lộ, dữ liệu cá nhân rò, sự cố nghiêm trọng | Owner quyết có bấm F1; agent phụ trách khôi phục, xoay vòng secret, sửa với khách | Sơ bộ 72 giờ hoặc 05 ngày làm việc, chính thức 15 ngày (NĐ 142 Đ.19); vi phạm dữ liệu 72 giờ (Luật 91/2025 Đ.23; GDPR Đ.33). Agent soạn, người đại diện ký; cơ quan nhận trước mọi postmortem công khai |
| C1 | Đầu ra sai đã giao; lời agent bị coi là cam kết | Agent phụ trách sửa với khách; auditor tìm đầu ra cùng kiểu; brief được xem lại | Như C0 nếu chạm ngưỡng "thiệt hại đáng kể" (câu hỏi 5) |
| C3 | Chất lượng trôi sau một thay đổi | Agent đã ra thay đổi: hoàn tác hoặc sửa tiếp | Phân loại lại nếu phát sinh rủi ro mới (Luật AI Đ.10(2)) |
| C2, C4 | Đúng kết quả sai quy trình; nợ (độ phức tạp, cảnh báo tĩnh, mở lại) | Agent hoặc role phụ trách | — |

Khi hành vi cần đổi, sửa brief hoặc playbook trước khi thêm flag (`AGENTS.md:48`); mỗi lỗi đã thoát thành một ca hồi quy. **Mỗi thay đổi là một release** có mã,
dòng change log, hoàn tác bằng một hành động; hồi quy chạy, kết quả gắn vào release và hiện trên màn hình agent đã sửa; release ra ngay, trừ hai ngoại lệ do luật:
release I12 "có" chờ chữ ký; ở bên áp dụng niêm yết tại Mỹ, release (kể cả prompt và cấu hình) đổi một con số vào báo cáo tài chính đi qua merge có bằng chứng
duyệt (SOX §404). **Đề xuất (giả thuyết):** mỗi role một release đang chạy hồi quy tại một thời điểm, để kết quả hồi quy gắn đúng một thay đổi.

**Nhịp đọc (lịch đọc, không phải giai đoạn):** liên tục (log, kiểm xác định, hàng đợi, hồi quy); mỗi tuần (scorecard, nợ); tuần 8 và 12 (xu hướng nợ,
2511.04427); tháng 3, 6, 9, 12 (S1–S8 và hành động nhà tài trợ đã ghi; không kết luận lợi tức trước tháng 9–12, 2607.01904). **Theo dõi vận hành:** W1 thời
gian phát hiện, dừng, khôi phục (M7); W9 owner bỏ sót (M5, lỗi thấy sau merge); W11 suy giảm sau release (phân phối theo `config_hash`, revert theo băm);
W12 nợ (M4, chain dọn nợ); W2 số agent so với việc phân rã được (thành công trên 1.000 token theo cell → brief của Chair); W3 tỷ lệ key result
`unverifiable` theo thời gian (→ brief contract-first); W4 mục tiêu tự đặt bị hạ (→ kiểm mẫu); W5 tỷ lệ bắt của checker, auditor, Chair trên lỗi gieo, kèm báo
nhầm và d′ (→ thêm checker, đổi panel); W6 ρ, n_eff trên tập neo (n_eff gần 1 thì dồn sang checker); W7 tỷ lệ đạt hồi quy so với khiếu nại và lỗi thoát 30
ngày (→ làm mới lát niêm); W8 hành động H2 có tham số từ nội dung ngoài (→ mở rộng SC1, nêu lại SC4); W10 surrogate lệch dấu với kết quả dài hạn (→ `expired`).

### 6.4 Scorecard (chỉ quan sát; không chỉ số nào tự kích hoạt dừng, thưởng hay phạt)

Định nghĩa mở; chỉ thành thước đo chung khi ≥ 2 tổ chức độc lập áp dụng và một bên tính lại số của bên kia khớp. **Kết quả đã xác minh:** đầu ra đã giao, đạt
kiểm kết quả, đạt kiểm quy trình, không bị thấy sai trong 30 ngày (cửa sổ 30 ngày là **Đề xuất (giả thuyết)**; không dùng cụm này trong marketing, FTC §5).

| # | Chỉ số | Định nghĩa | Nguồn định nghĩa |
|---|---|---|---|
| M1 | Nhất quán | Ca t chạy K lần, c_t lần đạt; pass^k = trung bình C(c_t, k)/C(K, k), báo k = 1 và 3; C_out = trung bình (2p̂_t − 1)², báo kèm tỷ lệ đạt; lần tệ nhất; tách lát mở và niêm. Ví dụ K = 5, c = 4: pass^3 = 4/10 = 0,40; C_out = 0,36 | τ-bench 2406.12045; Rabanser 2602.16666 |
| M2 | Quy trình | Strict pass = đạt kết quả **và** đủ bước bắt buộc trong log; false-pass = (đạt kết quả, sai quy trình)/(đạt kết quả) | CXBench 2610.25954 |
| M3 | Lỗi thoát | DER_30 = lỗi thấy lần đầu sau khi giao (≤ 30 ngày) / mọi lỗi thấy trên cùng đầu ra, theo mức nặng và lớp hiệu ứng | **Đề xuất (giả thuyết)** |
| M4 | Nợ | Cảnh báo tĩnh, độ phức tạp trên KLOC, cùng phiên bản công cụ, so với ngày 0; ngoài code: tỷ lệ làm lại 30 ngày | 2511.04427 |
| M5 | Tải lên người | Phút owner trên 100 kết quả đã xác minh; số việc chờ, tuổi p95; tỷ lệ chấp nhận; phân phối số bước giữa hai lần người can thiệp | 2512.04123; 2607.01904 |
| M6 | Chi phí trên kết quả đã xác minh | (hoá đơn model, compute + phút người × đơn giá có gánh) / kết quả đã xác minh; tài chính đọc ngoài hệ agent, không vào thưởng | R9; ADR-0010:43 |
| M7 | Phát hiện, phục hồi | Thời gian phát hiện (hiệu ứng → mục đầu tiên trên màn hình) theo lớp; thời gian dừng (F1 → byte ra cuối ở mạng host); thời gian khôi phục; cửa sổ mất dữ liệu | NIST AI RMF MANAGE 2.4 (tự nguyện) |

Quy tắc báo cáo: phân phối (p5, p50, p95) và lần tệ nhất (Vending-Bench: Claude 3.5 Sonnet trung bình $2.217,93, thấp nhất $476,00, tức 21%; 2502.15840 Bảng 1,
[BT]); mỗi tỷ lệ kèm n và khoảng Wilson, sai số theo cụm (2411.00640); mỗi điểm kèm mã model thực, mã release, cửa sổ, cách lấy mẫu; chuỗi thời gian có vạch
release, đọc trước và sau một vạch là gợi ý, không phải quy kết.

### 6.5 Quy tắc mở (R2)

**Với owner và người của công ty** mọi thứ mở. Ngoại lệ do owner chọn có **hai**: **giá trị** credential (thấy credential nào tồn tại, phạm vi, mọi lần dùng),
và **tập niêm** với agent bị đo, vì bộ kiểm mà bên bị đo thấy và dùng lặp thì bị quá khớp (1411.2664, 1502.04585); tập neo niêm là holdout của dữ liệu đánh giá,
không của khách. Luật buộc thêm giới hạn **dữ liệu cá nhân** theo mục đích. **Ra ngoài** chỉ sau bốn kiểm, mỗi kiểm một chữ ký: dữ liệu cá nhân (mã băm có thể
là bút danh); bảo mật hợp đồng (DPA, NDA); thứ tự báo sự cố (cơ quan nhận trước postmortem); thừa nhận trách nhiệm (báo cáo sơ bộ đúng hạn "không bị coi là sự
thừa nhận", postmortem công khai thì không, NĐ 142 Đ.19). Được ra: phương pháp, mã tính M1–M7, scorecard tuần với n và khoảng tin cậy, change log, postmortem đã
qua kiểm, kết quả xấu như tốt. Không ra công chúng (vẫn mở trong công ty), vì lý do an ninh: ngưỡng, vị trí canary, tập niêm, chi tiết harness chỉ chỗ cho kẻ tấn
công (2510.09023); ca công khai còn có thể vào dữ liệu huấn luyện (o3 xác định tệp lỗi tới 76% trên SWE-bench Verified, dưới 53% ở kho ngoài; 2506.12286, [BT]).
Chỉ phân phối lặp lại được: model đóng bị ngừng, tham số lấy mẫu bị loại ở API Anthropic 2026-10-21 ([P §4], MEDIUM).

### 6.6 Mẫu triển khai cho doanh nghiệp lớn

**Công tắc dừng:** agent, cell (owner đơn vị, hoặc Assistant theo tin nhắn đã xác thực); tenant, cả ranh giới (owner đơn vị, người chịu trách nhiệm cấp điều
hành); một nhà cung cấp model (hiện thực tham chiếu chỉ có một, ADR-0006; chuyển chỉ khi bên kia có DPA và eval riêng). **Ma trận kiểm soát** sang NIST AI RMF, NIST AI
600-1, ISO/IEC 42001 Phụ lục A, AI Act, OWASP LLM 2025: 16 dòng ở `council/04-launch.md` §5.4 (**Đề xuất (giả thuyết)**, cần kiểm toán viên xác nhận).

**Tuyên bố rủi ro còn lại có chữ ký** (mỗi dòng một người cấp điều hành nhận, qua tin nhắn đã xác thực; không thêm giới hạn lên agent, chỉ ghi ai chấp nhận
việc không có nó; câu hỏi luật sư 11 về hệ quả của chữ ký):

| # | Điều bản này cố ý không làm | Hệ quả | Người nhận (vai) |
|---|---|---|---|
| RR1 | Không giới hạn sáng kiến, không giới hạn phạm vi ngoài luật | Uỷ ban rủi ro thiếu "risk limits" (FSB 2013, MEDIUM) | Giám đốc rủi ro |
| RR2 | H2 trong ranh giới không có cận khi hoãn-có-huỷ tắt | Email, phát ngôn, dữ liệu rò, tiền đã chuyển không lấy lại | Tổng giám đốc |
| RR3 | Không Rule of Two, không danh sách đích | ASR có mục tiêu 47,69% (GPT-4o); thích ứng > 90% | Giám đốc an ninh thông tin |
| RR4 | Không trần chi phí; hệ không ghi chi phí | Multi-agent khoảng 15 lần token chat (Anthropic, MEDIUM); vòng "meltdown" | Giám đốc tài chính |
| RR5 | Không thí nghiệm: lợi tức không quy được nguyên nhân | Chỉ so sánh quan sát trong/ngoài ranh giới, có thiên lệch chọn | Giám đốc tài chính |
| RR6 | Cam kết với khách không bị chặn bằng mã | Ràng buộc công ty (Luật GDĐT Đ.14(2)(a)); trần trách nhiệm vendor bằng phí 12 tháng | Pháp chế |
| RR7 | Một nhà cung cấp model, một owner mỗi server (hiện thực) | Rủi ro tập trung (DORA); 40 lỗi trên 20 card trong 52 phút khi chạm giới hạn phiên (ADR-0005:13–15, [BT]) | Giám đốc công nghệ |
| RR8 | Bảo hiểm có thể loại trừ AI | Verisk/ISO loại trừ AI sinh cho CGL từ 01/01/2026 (MEDIUM); hồ sơ bảo hiểm ước phần không được bảo hiểm | Giám đốc tài chính |
| RR9 | Lát mở của bộ hồi quy agent thấy được | Điểm lát mở là cận trên lạc quan | Người giữ scorecard |
| RR10 | Chưa có SOC 2, ISAE 3402, pentest bên thứ ba | Kiểm toán thiếu chứng thực | Kiểm toán nội bộ |

**Triển khai tham chiếu** (cửa sổ dữ liệu, không phải giai đoạn): điều kiện vào là lỗ ở mục 7.2 đã đóng có biên bản, D1–D18 đạt, RR đã ký, hồ sơ "trước khi
dùng" đã nộp; M1–M7 công bố theo tuần sau bốn kiểm, **trong ít nhất 12 tháng**: quý đầu trả lời M1–M5 và M7, còn S1 và M6 chỉ đọc được ở tháng 9–12; M6 từ hoá
đơn; so trong/ngoài ranh giới trên D10 và M3, ghi rõ không ngẫu nhiên; báo hội đồng quản trị mỗi tháng (M1–M7, sự cố C0–C1, trạng thái RR, S1–S8 ở các mốc đọc,
hành động nhà tài trợ đã chọn), do người cấp điều hành ký, vì hội đồng phải có hệ thống giám sát và báo cáo cho chức năng mission-critical (Marchand v. Barnhill
2019, MEDIUM); gói hội đồng lao động (dữ liệu về người được ghi, mục đích, thời hạn lưu, ai truy cập).

### 6.7 Pre-mortem: một năm sau đã thất bại (chín cách)

| # | Cách thất bại | Biện pháp | Phủ |
|---|---|---|---|
| 1 | Bấm dừng mà agent vẫn chạy (phiên còn mạng ngoài gate, `setsid` sống tiếp), hoặc credential ngoài kho bị đọc | D1 ở mạng host; D2 quét mọi credential; mục 7.2 | Đo được bằng D1, D2; đầy đủ là kết quả diễn tập, chưa có |
| 2 | Injection khiến agent dùng connection hợp lệ gửi dữ liệu hay cam kết ra ngoài | SC1, I19, SC6, F2 | Một phần trước hành động (SC1); còn lại chỉ phát hiện và bù |
| 3 | Hành động H2 hàng loạt (email, hoàn tiền chia nhỏ, phát ngôn) | Tổng theo cửa sổ; SC4 nếu bật; H2 ngoài ranh giới đầu tiên | **Không có cận** khi H2 ở trong và SC4 tắt |
| 4 | Hàng đợi owner sập: merge là rail, agent không giới hạn, owner duyệt máy móc | Trần ADR-0001 (mặc định giữ); M5 tuổi p95 và tỷ lệ chấp nhận từ tuần 1; bằng chứng do nền tảng dựng | Một phần |
| 5 | Không trả lời được "có hiệu quả không" ở tháng 4–6; nhà tài trợ rút | M6 do tài chính đọc; so trong/ngoài ranh giới; hành động ở mỗi mốc ghi trước | Một phần: không quy nhân quả được |
| 6 | Pháp chế buộc thêm duyệt sau khi bật | F3 thắng; câu hỏi 1–3 trả lời trước ngày 0 | Phụ thuộc luật sư |
| 7 | Scorecard xanh, khách đỏ: quá khớp với bộ kiểm agent thấy | Lát niêm, tập neo niêm, D10 cạnh mọi chỉ số | Một phần |
| 8 | Số sai âm thầm tới khách trước auditor (auditor LLM mù cùng chỗ) | Tool tính số; tỷ lệ bắt đo trước ngày 0 | Một phần |
| 9 | Chức năng bị thay quay lại qua kênh khác; thay người sai thủ tục (Klarna, CBA); thiếu thông báo trước khi dùng, báo sự cố trễ, công bố lọt dữ liệu | D10; lịch luật lao động; D11–D14; nghĩa vụ báo chạy độc lập với F1 | Phủ các mục đã biết; chức năng: một phần |

## 7. Hiện thực tham chiếu: Remit, tiến hoá từ Remit (R7)

Remit là hiện thực tham chiếu, **không phải bằng chứng hiệu quả**.

### 7.1 Có gì, và cái gì là yêu cầu chưa đạt (HIGH khi dẫn mã; MEDIUM khi chỉ dẫn ADR hay `AGENTS.md`)

Credential owner nhập vào trường chỉ ghi của một mục inbox; Chair chỉ nêu tên biến (`internal/orch/credentials.go:20`, HIGH). **Sealed** và **One gate**
(ADR-0018:38–46) là **yêu cầu chưa đạt**: phiên còn mạng ngoài gate, đọc được khoá SSH, đăng nhập GitLab, môi trường tiến trình (mục 7.2). Agent cầm công cụ Remit viết và công cụ của phần mở rộng owner đã Apply (ADR-0017, thu hẹp bởi ADR-0029 và ADR-0034), cộng chương trình và máy chủ MCP trên
stdio, vốn chạy được qua `run` sau một grant (ADR-0018:62–63), nên bề mặt B3 hẹp hơn ở công cụ gõ sẵn, không hẹp hơn ở chương trình. Pause và stop, merge do
owner là rail; chain giữ kết quả tới khi hiệu ứng được goal judge xác nhận (ADR-0013); khách hàng mở đầu chain (ADR-0016); thay đổi do chain tự sửa chỉ tới chain
sau khi owner merge (ADR-0012); không ghi và không trần chi phí (ADR-0010; với bên áp dụng, đây là lỗ M6 phải bù bằng tài chính đọc hoá đơn); trần N = 5 MR chờ
owner, "a rail: no agent raises it" (ADR-0001:20, 28); jail có adapter theo hệ điều hành (ADR-0021). Các mục chỉ dẫn ADR là MEDIUM.

Một khoá owner mỗi server, không người dùng, không vai trò, không đăng nhập (ADR-0026, `remit-design.md` K3): các chữ ký của nhiều người cấp điều hành (RR, D11, D12) mà bên
áp dụng cần là việc của bên áp dụng ngoài Remit; trong Remit một chữ ký là Apply một change item của owner cộng băm nội dung được ký (K9). Phần mở rộng là tiến trình
`go-plugin` do khách hoặc chain cung cấp và owner Apply: ghim bằng SHA-256, không thừa kế môi trường máy chủ, công cụ của nó đi qua cổng như công cụ nguồn thường
(ADR-0034, `remit-design.md` K5; kiểm soát bao bọc tiến trình plugin ở *Revisit when* của ADR-0034). Nền tảng không kiểm chứng được phần mở rộng làm gì so với điều nó khai.

### 7.2 Lỗ đã biết: việc xây trước ngày 0, không phải sự thật (chưa có ngày đóng)

| Lỗ còn mở trong code | Nguồn (`file:line`) | Invariant | Việc phải xây | Phép thử |
|---|---|---|---|---|
| Phiên giữ mạng của máy tới nhà cung cấp model và mọi nơi khác, ngoài gate: proxy egress chỉ chạy cho chương trình và script | `internal/jail/jail.go:9`; `internal/claudecode/conn.go:171`; `internal/orch/program.go`, `script.go` (nơi duy nhất dùng `egress`); ADR-0018:71–72 | I1, I5, I17 | Mọi lưu lượng của phiên qua một proxy của nền tảng (SC2; `remit-design.md` K2, U2) | D1 ở mạng host |
| Hồ sơ Seatbelt của phiên macOS bắt đầu bằng `(allow default)`; jail của chương trình đã là `(deny default)` | `internal/jail/sandbox_darwin.go:143` (phiên) và `:125` (chương trình) | I1 | Phiên theo mẫu của jail chương trình: mặc định từ chối, mạng chỉ qua proxy | Thử thoát sandbox |
| Phiên không có pid namespace: tiến trình con `setsid` sống quá deadline (macOS), và phiên báo hiệu được cho mọi tiến trình cùng user, server kể cả (cả hai OS) | `internal/jail/plan_linux.go:37–40` (pid và network namespace chỉ khi `p.Jail != nil`); ADR-0021:63; ADR-0018:109–111 | I5 | `CLONE_NEWPID` cho phiên trên Linux; user OS riêng cho phiên (K2) | D1: không còn tiến trình con |
| macOS: phiên đọc môi trường của server và mọi tiến trình cùng user (`kern.procargs2`); trên Linux user namespace đã che | `internal/claudecode/environ_darwin_test.go:6` (`hidesEnviron = false`), `environ_linux_test.go:5` (`true`); ADR-0021:66 | I2 | User OS riêng; môi trường server không chứa secret | D2 |
| Phiên cùng user OS với server: ghi được crontab và tệp khởi động shell của user đó | ADR-0018:109–112; ADR-0004; không có đường chạy tiến trình dưới user khác trong code (`rg -n 'os/user\|syscall\.Credential' cmd internal --glob '*.go'` rỗng) | I2, I5 | User OS riêng cho phiên (K2, U2) | Ghi crontab của user server từ phiên: bị từ chối |
| Khoá SSH và đăng nhập GitLab (`~/.ssh`, `~/.config/glab-cli`) đọc được từ phiên và từ lệnh kiểm; forge chạy bằng `glab` đã đăng nhập trên máy | ADR-0018:112–114; `internal/forge/gitlab.go:14`; `internal/source` không có kiểu kết nối GitLab | I2 | Kiểu kết nối GitLab sau gate | D2: 0 kết quả |
| Gate chưa có mã `Stopped` và `Law`: F1 chưa là một lý do từ chối, F3 chưa có `LegalPolicy` | `internal/grant/grant.go:65–70` (chỉ `NoGrant` và `Rail`) | I1, I5, I17 | `StopView` và `LawView` quanh `grant.Decide` (K4; U2, U9) | `Resolve` trả `Stopped` cho mọi phạm vi đang dừng |
| Sổ gọi là một bảng do server ghi, chưa có chuỗi băm | `internal/orch/source_calls.go:12–14`; `remit-design.md` K4 (hoãn) | I4 | Chuỗi băm khi owner yêu cầu | Kiểm chuỗi băm hằng ngày |
| Một nhà cung cấp model (Claude Code) | `cmd/remit/agents.go:46` (`agent.ClaudeLauncher`); ADR-0006 | — | Judge nhiều nhà cung cấp và backend theo cell (K7); hoặc chữ ký RR7 | Diễn tập chuyển, hoặc RR7 đã ký |
| "Least grant" và grant `write` chờ owner (ADR-0018 invariant 2), việc xét phép kiểm trước khi làm (ADR-0013:37), trần N = 5 (ADR-0001) đang chạy; owner chưa quyết giữ hay bỏ | ADR-0018:40–43; `internal/grant/grant.go:69` (`NoGrant`); ADR-0013:37; ADR-0001:20; `internal/orch/admit.go:37`; `remit-design.md:243` | I17 | Theo quyết định 2–4 | — |

**Hệ quả:** hôm nay F2 **bị vi phạm** trên hệ tham chiếu (HIGH) và F1 không phủ đường ra ngoài gate; mọi khẳng định về F1, F2 là MEDIUM cho tới khi các lỗ trên
đóng và diễn tập đo được. Không có SOC 2 hay pentest bên thứ ba. Phần lớn lỗ là đặc thù macOS và một host một user.

## 8. Sổ bằng chứng

### 8.1 Bốn mươi khẳng định chịu lực

Cột "Kiểm": BT = biên tập tra lại; A1 = kiểm toán số liệu độc lập của hội đồng cũng xác nhận; "—" = chưa ai tra trong nguồn gốc.

| ID | Khẳng định, con số | Nguồn | Nhãn | Kiểm |
|---|---|---|---|---|
| E01 | Một công ty, panel 802 dev (ước lượng trên 564), 196.212 PR; +12% lúc bắt đầu dùng; 2× sau khoảng 9 tháng dùng; mục tiêu đạt tháng thứ mười; tải mỗi reviewer gấp đôi | 2607.01904 | HIGH | BT, A1 |
| E02 | Dòng thêm 3–5× tháng đầu, tan sau hai tháng; cảnh báo +30,26% (±6,66%), độ phức tạp +41,64% (±7,62%) | 2511.04427 | HIGH (mã nguồn mở) | BT, A1 |
| E03 | Reliability +0,03/năm (GAIA), +0,09/năm (τ-bench); accuracy +0,23, +0,22/năm; 15 model, 24 tháng | 2602.16666 | HIGH | BT, A1 |
| E04 | Độ dài tác vụ phần mềm ở mức 50% gấp đôi khoảng 7 tháng (131 ngày gần đây: MEDIUM, blog) | 2503.14499 | HIGH | BT, A1 |
| E05 | Agent tốt nhất tự hoàn thành 30,3% tác vụ công ty mô phỏng (điểm 39,3%); DS, hành chính, tài chính thấp nhất | 2412.14161 | HIGH | BT, A1 |
| E06 | Dự án freelance thật: tỷ lệ tự động hoá cao nhất 2,5% | 2510.26787 | HIGH | BT, A1 |
| E07 | CEO-Bench: mọi model dưới baseline quy tắc ($15,8M); lần chạy tốt nhất của model tốt nhất $12,6M; khởi đầu $1M | 2606.18543 | HIGH (mô phỏng) | BT, A1 |
| E08 | Bảng tính tài chính: tốt nhất 82,4% (khoảng 1/6 sai); bảng lớn nhất 48,6% so với 86,2% | 2603.07316 | HIGH | BT, A1 |
| E09 | 55,5% của 840 lần chạy false-pass = 80,1% của lần đạt kết quả; 13,8% đạt cả hai | 2610.25954 | HIGH | BT, A1 |
| E10 | gpt-4o pass^8 dưới 25% ở bán lẻ | 2406.12045 | HIGH | BT, A1 |
| E11 | +80,8% (suy luận tài chính phân rã được, tập trung) đến −70,0% (lập kế hoạch tuần tự); SWE-bench Verified −15% đến −2% mọi kiến trúc nhiều agent | 2512.08296 | HIGH | BT, A1 |
| E12 | Khuếch đại lỗi 17,2× (độc lập) so với 4,4× (tập trung); ngang hàng 0,477 so với 0,466; 23,9 so với 67,7 thành công/1.000 token | 2512.08296 Bảng 5 | HIGH | BT, A1 |
| E13 | Quyền gửi lại hại khi chỉ góp ý, có ích khi "can verify"; 43 cặp, 86 lần chạy | 2609.14767 | HIGH số; MEDIUM khái quát | BT, A1 |
| E14 | Review nhiều vòng: +62% báo nhầm (8,47 so với 5,23) | 2603.16244 | HIGH (một tác giả) | BT, A1 |
| E15 | 353 PR bị từ chối trong mẫu 717: 35,7% lỗi rõ, 31,2% quy trình, 33,1% không lý do | 2605.22534 | HIGH | BT, A1 |
| E16 | Xung đột 48/115 cặp PR khác agent so với 119/601 cùng agent; 27,67% PR agent có xung đột merge | 2607.04697; 2604.03551 | HIGH | BT, A1 |
| E17 | 9 judge, 7 họ = 2,18 phiếu độc lập | 2605.29800 | HIGH số; MEDIUM khái quát | BT, A1 |
| E18 | Ba judge frontier khác nhà cung cấp: 91–93% chính xác, ρ = 0,56, cùng sai 7,7×, n_eff 1,41; mười open-weight n_eff 3,51 | 2609.22512 | HIGH | BT, A1 |
| E19 | Một lần đổi phiên bản judge âm thầm thấy ở 60/60 lần chạy; z-test báo nhầm 75% | 2606.15474 | HIGH số; MEDIUM khái quát | BT, A1 |
| E20 | Đổi judge: 59% tập top-30% đổi; 93% điểm vào một giá trị | 2610.32986 | HIGH | BT, A1 |
| E21 | Không monitor 83/88 (94%) không thấy phá hoại; monitor báo 16/19, phá hoại vẫn thành công 9/16 | 2606.05647 | HIGH (đối kháng) | BT, A1 |
| E22 | Chuyển ca đã gắn cờ: TPR +15% ở FPR 0,01; "lack of adversarial robustness for LLMs and humans" | 2508.19461 | HIGH | BT, A1 |
| E23 | ASR có mục tiêu 47,69% (GPT-4o), 33,86% (Claude 3.5 Sonnet) trên AgentDojo | 2406.13352 | HIGH | BT, A1 |
| E24 | Tấn công thích ứng vượt 12 phòng thủ, > 90% cho đa số | 2510.09023 | HIGH | BT, A1 |
| E25 | Tách luồng không chính sách: Gemini 2.5 Pro 300 → 0 trên 949; CaMeL 77% so với 84% tiện ích | 2503.18813 | HIGH | BT, A1 |
| E26 | Gắn nhãn nguồn đứng riêng chỉ giảm ASR 5% | 2410.07283 | HIGH | BT, A1 |
| E27 | Đầu độc công cụ: ASR cao nhất 72,8%; đổi tham số 46,7% | 2508.14925 | HIGH | BT, A1 |
| E28 | Gián tiếp 27,1% so với trực tiếp 5,7%; khoảng 1,8 triệu tấn công | 2507.20526 | HIGH | BT, A1 |
| E29 | Phá cơ chế tắt tới 97% (96–98%) dù được dặn; > 100.000 lượt, 13 model | 2509.14260 | HIGH (kịch bản dựng) | BT, A1 |
| E30 | 86 hệ đang chạy thật hoặc thử nghiệm, 306 người: 68% của 60 hệ trả lời ≤ 10 bước trước khi người can thiệp; 74% dựa vào đánh giá người | 2512.04123 | HIGH (mẫu quen biết) | BT, A1 |
| E31 | Ví dụ tính (σ² = 1/6, ω² = 1/9, n = 198): K từ 1 lên 10 giảm MDE 13,2% → 7,5%; K ≥ 5 là lựa chọn của hội đồng | 2411.00640 §5 | HIGH (ví dụ) | BT, A1 |
| E32 | Vending-Bench, Claude 3.5 Sonnet: trung bình $2.217,93, thấp nhất $476,00 | 2502.15840 | HIGH | BT, A1 |
| E33 | Bing: lỗi giảm chất lượng, truy vấn mỗi người > +10%, doanh thu mỗi người > +30% | Kohavi 2012 | HIGH | BT, A1 |
| E34 | "No substitute for a well-run long-term experiment" (tác giả); thông báo mỗi 7 ngày là ví dụ giả định của một người tham gia; surrogate Netflix 79% precision, 65% recall so với quyết định theo số đo trực tiếp tháng thứ hai | 2608.08043; 2311.11922 | HIGH | BT, A1 |
| E35 | Gần 9.000 lãnh đạo, 7 nước: năng suất +0,2%/năm (kỳ vọng 0,6%) | NBER w34836 | HIGH (tự báo cáo) | BT, A1 |
| E36 | Contract-first: giao việc chỉ khi kết quả "has precise verification" | 2602.11865 | HIGH | BT, A1 |
| E37 | Dogfooding: $1.126,07, 3,5 ngày, khoảng 114 chain, 8 MR, $141/MR, $8,25/chain merge; 83% ($933) tự giao; 52 MR chờ | ADR-0002:25–26, 38–40 | HIGH (n = 8) | BT, A1 |
| E38 | 40 lỗi trên 20 card, 13:48–14:40, 26/10/2026 | ADR-0005:13–15 | HIGH | BT, A1 |
| E39 | Lỗ hệ tham chiếu (mục 7.2) | ADR-0018, ADR-0021, `sandbox_darwin.go:143` | HIGH | BT, A1 |
| E40 | Knight Capital lỗ > 460 triệu USD trong khoảng 45 phút | SEC qua [E §5] | MEDIUM (chưa ai mở lại trang SEC) | — |

Nguồn không phải PDF, đọc qua phản biện, chưa mở lại: Klarna (2,3 triệu chat, 2/3 số chat, ước +$40 triệu 2024, HIGH[C01] thông cáo; tuyển lại người 05/2025,
MEDIUM); hai hậu kiểm Anthropic (mục 3.9, HIGH[C01]); PocketOS (9 giây, backup cùng volume, MEDIUM); Replit (nguồn mâu thuẫn về rollback, OECD ghi mất dữ liệu
vĩnh viễn, MEDIUM); Devin 67% (MEDIUM vendor); Paperclip README, IMDA (HIGH[C06]); Wei, Karpathy (MEDIUM, chỉ qua kết quả tìm kiếm). Luật ở mục 4: HIGH[C04].

### 8.2 Khẳng định chúng tôi không đưa ra

| Khẳng định phổ biến | Vì sao không |
|---|---|
| "Agent cắt 80% chi phí review/QA" | Không có nghiên cứu; dữ liệu đi ngược (E01, E02) |
| "95% dự án GenAI thất bại" (MIT NANDA); ARR của công ty "do agent vận hành" (Polsia, NanoCorp, Medvi); tỷ lệ giải quyết của vendor | Bị phê bình phương pháp, hoặc tự báo cáo không mẫu số, không kiểm toán [P §1.1, §2, §3] |
| Một con số ROI, hoặc "$141 mỗi MR" làm chuẩn chi phí | n = 8, dogfooding, 83% tự giao, cơ sở chi phí chưa rõ (E37) |
| "Model khác họ cho auditor là đủ độc lập"; "chi phí agent giảm như cloud"; "Klarna chứng minh AI thay hỗ trợ khách" | E17, E18; giá ở biên frontier không giảm [D §8]; CEO Klarna nói "lower quality", tuyển lại người |
| "Người duyệt cuối là lưới an toàn"; "nút dừng là đủ kiểm soát của con người"; "tổn thất có cận vì có phục hồi" | E21, E22; Luật AI Đ.4(2); sai với H2 (mục 3.8) |
| "Luận điểm 'mua sinh, xây kiểm chứng' là của chúng tôi"; "niêm credential là điểm khác biệt" | Wei, Karpathy, 2602.11865; Auth0 Token Vault, Arcade, AWS AgentCore Identity |
| "Chỉ CaMeL có số đo chống injection"; "scorecard sẽ thành tiêu chuẩn chung"; tầm "nhân loại" | FIDES 2505.23643 cũng đo trên AgentDojo (MEDIUM); cần bên thứ hai áp dụng (đã có AIUC-1, HAL); không có bằng chứng ở mức nào |
| "Bật một lần là cách đã được chứng minh an toàn" hoặc "đã được ghi nhận là mẫu thất bại" | Big bang là mẫu đã biết; một ca thất bại nổi tiếng (Hershey 1999), không có tỷ lệ nền |
| "Bằng chứng ủng hộ bỏ giới hạn quyền" | Không có số đo nào; mọi khung [C06] tìm thấy khuyên ngược lại (mục 1) |

## 9. Sổ rủi ro tóm tắt

Đầy đủ ở `council/03-ledger.md` §3 (đánh số R0–R18 ở đó; ở đây RK để khỏi trùng R1–R10). Lớp hiệu ứng theo quy tắc mục 3.8.

| # | Rủi ro | Lớp | Đúng ở ngày 0 | Nhận ra và sửa | Còn nhận |
|---|---|---|---|---|---|
| RK0 | Bật cùng lúc, không so sánh đồng thời bên trong | Cả ba | Mọi role chạy; rail có; số khách chỉ đổi qua merge | Chuỗi thời gian theo tenant và `config_hash`; so với bên ngoài | Biết **có** gì đổi, thường không biết **vì sao** |
| RK1 | Số tài chính sai tới khách âm thầm | H1 (quyết định khách đã ra: H2) | Phép tính là mã có phiên bản, đổi qua merge | Auditor đối chiếu dữ liệu thô; khiếu nại; phát lại; báo khách | Prompt, cấu hình đổi con số hiển thị; auditor LLM mù cùng chỗ |
| RK2 | Hành động phá huỷ | H0 nếu backup ngoài tầm; H2 ở bên thứ ba | F2 chỉ đúng với giá trị trong kho; lỗ mục 7.2 | Log nền tảng; dừng; khôi phục | Xoá ở hệ bên thứ ba |
| RK3 | Cam kết ràng buộc với khách | H2 (khoản bù bằng tiền là H1) | Mọi phát ngôn là của pháp nhân; "đây là AI" | Auditor đọc bản ghi cam kết; C1; thực hiện cam kết; sửa brief | Cam kết về giá chạm ICFR |
| RK4 | Dữ liệu khách vào nhà cung cấp LLM | H2 | DPA, SCC; sổ mục đích; hồ sơ 60 ngày | Đối chiếu luồng thật với hồ sơ | Luồng đi ngoài gate trên hệ tham chiếu |
| RK5 | Injection, confused deputy, chuỗi cung ứng công cụ | H2 | F2 che credential trong kho; SC1 bật (mặc định) | Log nguồn gốc theo tham số; auditor; dừng | Dùng công cụ hợp lệ cho việc trái phép |
| RK6 | Agent can thiệp monitor, log, eval, nút dừng | H0 nếu log ngoài tầm | F1 ngoài tầm agent; mã dừng chỉ đổi qua merge | So log nền tảng với lời agent; dừng đo ở mạng host | Đường ra ngoài gate, `setsid` (mục 7.2) |
| RK7 | Owner không thấy kịp, dừng muộn; một người giữ mọi vai (điểm đơn lẻ) | Quyết định thời gian dừng ở mọi lớp | Hàng đợi ngoại lệ do máy xếp; trần ADR-0001 | Thời gian phát hiện đo bằng lỗi gieo; T và người trực công bố | Chưa có nghiên cứu một owner theo dõi nhiều giờ |
| RK8 | Model, nhà cung cấp đổi, suy giảm, ngừng | Cấu hình H0; đầu ra đã giao theo lớp của nó | Mã model thực mỗi bước; tập neo niêm | So phân phối theo mã model và băm prompt | Suy giảm cả hai phía không bắt; tập trung vendor |
| RK9 | Judge, auditor sai cùng chiều với agent | Theo việc | Auditor khác pipeline; ρ, n_eff đo, không giả định | Lỗi gieo; so với neo | n_eff trên dữ liệu tài chính chưa biết |
| RK10–11 | Pháp lý; kết quả không lặp lại giữa các lần chạy | Theo việc | Mục 4; lặp K lần là quan sát | Câu trả lời luật sư là điều kiện bật; đọc phân phối, không một lần | Đ.4(2) có thể đòi hơn nút dừng; lần tệ nhất ẩn dưới trung bình |
| RK12 | Nợ code tích tụ | H0, tốn kém | Mốc cảnh báo tĩnh, độ phức tạp ngày 0 | Xu hướng; chain dọn nợ | Hai nguồn kéo hai hướng về thời điểm đọc |
| RK13 | Tối ưu proxy thay mục tiêu | H1 đến H2 (khách đã rời) | Detector, mục tiêu tự đặt không vào thưởng | Giữ chân theo nhóm; khiếu nại | Không tách được khỏi mùa vụ |
| RK14–15 | Bộ nhớ, ngữ cảnh chung hỏng dần, bị đầu độc; nhiều agent ghi chồng | H0 | Mục có schema, phiên bản; ghi cũ bị từ chối kèm diff | So phiên bản, quay về; tỷ lệ xung đột | Hỏng về nghĩa, đầu độc chậm, xung đột ngữ nghĩa |
| RK16 | Vòng lặp vô ích kéo dài | H2 (tiền đã trả cho nhà cung cấp) | Không trần chi phí; rate limit nhà cung cấp là trần thật (E38) | Owner thấy trên màn hình; F1; tài chính đọc hoá đơn | Tiền đã tiêu không lấy lại |
| RK17 | Agent tự thử trên khách hoặc dùng dark pattern | H2 | "Không thí nghiệm" áp cho cả agent; DSA Đ.25; FTC | Log nhóm nào nhận thay đổi nào | Sửa trong lúc chạy đổi trải nghiệm không báo trước |
| RK18 | Lỗ của hiện thực tham chiếu | Thừa hưởng RK2, RK4–RK6 | Lỗ mở (mục 7.2) | Diễn tập ở mạng host; quét mọi credential | F1, F2 MEDIUM tới khi đóng |

**Năm khẳng định rủi ro nhất:** "F2 đúng ở ngày 0" (sai trên hệ tham chiếu); "nút dừng là đủ kiểm soát của con người" (E39, E21, Đ.4(2)); "không giới hạn tương
thích với F3" (năm điều luật đòi); "tổn thất có cận nhờ phục hồi" (sai với H2); "quan sát đủ để biết sửa đúng" (LOW nếu không có đơn vị so sánh được).

## 10. Điều chưa biết, và cách học bằng quan sát (không thí nghiệm)

Học bằng: lặp tác vụ đã ghi, phát lại, lỗi gieo vào việc lịch sử và neo do người gán nhãn (quan sát theo R3), phân tích biến thiên đã có, và so với đơn vị ngoài
ranh giới (bán thực nghiệm). Cột cuối là điều quan sát một mình không tách được.

| # | Chưa biết | Quan sát cho thấy | Không tách được |
|---|---|---|---|
| U1 | Owner bỏ sót bao nhiêu sự cố cần dừng | Thời gian sự cố → dừng; thời gian nhận ra lỗi gieo | Sự cố không ai thấy |
| U2 | Auditor bắt được bao nhiêu số sai | Tỷ lệ bắt lỗi gieo; số sai khách báo trước auditor | Lỗi khác hình dạng lỗi gieo |
| U3 | Panel judge độc lập đến đâu trên dữ liệu kinh doanh | ρ, n_eff trên tập neo niêm | ρ trên dữ liệu thật khác neo |
| U4 | Agent tự đặt OKR có chọn mục tiêu dễ không; từ chối kèm lý do có hội tụ nhanh hơn không | Khoảng cách mục tiêu–kết quả; số vòng, việc làm lại | Mục tiêu dễ hay agent giỏi; không so được khi chỉ có một cách chạy |
| U5 | Tác động lên giữ chân và doanh thu | Chuỗi thời gian theo nhóm, `config_hash`; DiD với bên ngoài | Tách khỏi mùa vụ và thiên lệch chọn |
| U6 | Nợ tăng nhanh đến đâu | Xu hướng từ mốc ngày 0 | Mức "bình thường" trước ngày 0 |
| U7 | Role ngoài code làm được bao nhiêu | Kết quả từng role | Phần thành công nhờ người can thiệp ngầm (CBA) |
| U8 | Chi phí của cam kết sai; agent có né hay sửa monitor khi biết nó tồn tại | Tổng giảm giá, tín dụng theo role; thay đổi trên thứ đo trong log | Phần nào do agent chiều ý; né mà không sửa |
| U9 | Thời gian và chi phí dựng bộ kiểm chứng đạt một mức bắt lỗi; `F0` | Tỷ lệ bắt theo tuần trên mỗi lớp việc; chi phí dựng thật | Phần nào do model tốt lên |
| U10 | Các đầu vào ROI (`s`, `a`, `c`, `H`, `r`, `R`, `D`, `E[L]`, `G`, cơ sở của $141) | Chi phí và kết quả từng lớp việc theo thời gian | Nhân quả của lợi ích |
| U11 | Có ai vận hành công ty đầu cuối bằng agent với lợi nhuận được xác minh | Kết quả của chính hệ này nếu công bố cùng phương pháp | — |

## 11. Quyết định của owner

Mỗi mục: **mặc định của chủ tọa, chờ owner** (thân bài viết theo mặc định đó) và hệ quả của từng câu trả lời, trừ mục 5, đã là quyết định của owner. Quyết định đã ghi của
owner (2026-11-02; `remit-design.md` mục 10, ADR-0026): một khoá owner, không người dùng, không vai trò, không đăng nhập; mã đóng; cập nhật tự động; không cam kết
tương thích; Git là nền cho mọi việc; bỏ qua sao lưu và khôi phục dữ liệu điều phối; phân phối ra ngoài HOLD, chưa có khách. "Least grant" của ADR-0018 và trần
ADR-0001 còn mở (`remit-design.md:243`): mục 2 và 3 dưới đây.

1. **Rail merge (R1).** Mặc định: giữ, merge do owner. *Giữ:* mã nút dừng chỉ đổi qua merge (I16); owner duyệt mỗi thay đổi mã; ở doanh nghiệp, một owner
   duyệt mọi merge là tách nhiệm vụ của một người (câu hỏi 7). *Bỏ:* agent sửa được chính mã dừng (ADR-0012), F1 mất cơ sở.
2. **Trần ADR-0001 (N = 5 MR chờ owner) và giới hạn đếm ADR-0010.** Mặc định: **giữ** như rail bảo vệ owner: không có nó hàng đợi merge không có cận (52 MR chờ
   khi đo, ADR-0002:40) và người duyệt kém đi khi khối lượng lớn (E21, E22); nó giới hạn việc chờ người, không giới hạn việc agent làm. *Bỏ:* chi phí mỗi đơn vị
   chấp nhận tăng không trần (mục 2.4); owner duyệt máy móc hoặc ngừng duyệt (pre-mortem 4).
3. **ADR-0018 invariant 2 (Least grant; grant `write` chờ owner).** Mặc định: **thay** bằng ADR mới: grant là dữ liệu ghi lại, hiện trực tiếp, không chờ owner;
   dữ liệu cá nhân theo sổ mục đích (I17) vì luật đòi. *Giữ:* có một cổng duyệt trên sáng kiến; câu "không tự đặt cổng duyệt" ở mục 1 không còn đúng. Hôm nay `NoGrant` là mã từ chối trong code (`internal/grant/grant.go:69`) và quyết định còn mở (`remit-design.md:243`).
4. **ADR-0013:37–38 (phép kiểm được xét trước khi làm).** Mặc định: **giữ việc xét, bỏ việc chờ**: goal judge xét song song; kết quả chỉ tính "đã xác minh" khi phép
   kiểm đã qua xét. *Giữ cổng:* việc chờ goal judge, trái "kiểm chứng không chặn". *Bỏ việc xét:* chain tự đặt thước đo rồi đạt nó, không chứng minh gì.
5. **Công bố ra ngoài (R2; quyết định của owner).** Remit là mã đóng, chưa có khách, phân phối ra ngoài HOLD (`remit-design.md` K11): chưa có công bố ra ngoài. Trong công ty
   mọi thứ mở với owner; ngoại lệ là credential và tập niêm. Khi phân phối rời HOLD, mọi công bố đi qua bốn kiểm ở mục 6.5. *Mở hết ra ngoài:*
   ngưỡng, canary, tập neo vô hiệu (2510.09023); rủi ro DPA, NDA, thừa nhận. *Không công bố gì (hôm nay):* câu đóng góp (mục 1) là dữ liệu nội bộ.
6. **Đo dụng cụ là quan sát (R3).** Các loại trừ "seeded checks, pre-registration, statistical analysis plans, performance tripwires" là **cách chủ tọa đọc** lời
   owner "không có thử nghiệm gì hết, làm là chạy luôn, không có bí mật hoặc bất cứ giới hạn nào hết", **không phải lời owner**. Mặc định: lỗi gieo vào tác vụ
   lịch sử đã ghi, neo do người gán nhãn, lặp K lần là quan sát, không chạm khách. *Không:* S5, W1, W5, U1, U2 không đo được; câu đóng góp (a) mất.
7. **Chiến lược nhận dạng, mức đọc, hành động dự định công bố trước ngày 0.** Mặc định: công bố; không kích hoạt tự động (mục 2.6). *Không:* kết luận là đọc sau
   sự việc, dễ chọn mốc có lợi.
8. **Tách luồng điều khiển khỏi dữ liệu (SC1).** Mặc định: **bật** cho Chair và mọi role có công cụ H2 (D17). *Bật:* cùng FIDES, biện pháp có số đo đứng trước H2
   (E25); tiện ích giảm (77% so với 84%); đọc cách ly là giới hạn khả năng có tên (mục 5.2). *Tắt:* pre-mortem 2 không phủ trước hành động; RR3 mang thêm phần này.
9. **Hoãn-có-huỷ cho H2 (R5).** Mặc định: **tắt**. Red team [C03 D4], giám đốc rủi ro [C05] và người đọc lạnh của hội đồng khuyên
   **bật** cho H2 có tham số từ nội dung ngoài (I19) hoặc vượt tổng cửa sổ (SC6). *Bật:* H2 đó có cận đo được (cửa sổ cộng thời gian dừng), đổi lại một độ trễ
   công bố; không ai phải duyệt. *Tắt:* RR2 đúng như viết; không gì đứng trước một H2 ngoài SC1.
10. **Tripwire chỉ báo (SC6).** Mặc định: bật, chỉ báo, không bao giờ vào `Resolve`. *Không:* hoàn tiền, email chia nhỏ chỉ lộ khi owner tự nhìn tổng.
11. **Tổn thất không cận ở H2 trái `AGENTS.md:21`.** Owner chọn: (a) sửa `AGENTS.md:21` cho H2 (nhận tổn thất không cận, ký RR2); (b) bật hoãn-có-huỷ (quyết
    định 9); (c) giữ H2 ngoài ranh giới. Mặc định: **(c) cho ranh giới đầu tiên**; ranh giới do bên áp dụng chọn (R10) nên đây không là giới hạn trên agent bên
    trong; thêm H2 sau đó cần (a) hoặc (b). *Hệ quả của (c):* chưa đo được agent trên việc nói với khách, tiền và dữ liệu ra ngoài.
12. **Ngân hàng, tổ chức tài chính.** Mặc định: **ngoài** thị trường cho cấu hình không giới hạn; **trong** cho lớp đo và kiểm chứng (mục 0), giới hạn của họ giữ
    nguyên (FSB, DORA, SR 26-2; R4). *Có cho cả cấu hình:* quy định ngành buộc giới hạn quay lại, phần lớn lập trường gate không áp được.
13. **Đọc chi phí (R9; ADR-0010:43).** Mặc định: tài chính bên áp dụng đọc hoá đơn hằng tuần, ngoài hệ agent; không trần. *Nền tảng ghi chi phí:* đổi ADR-0010.
    *Bên áp dụng đặt trần ở tài khoản nhà cung cấp:* là giới hạn sáng kiến, ngoài cấu hình này. *Không ai đọc:* pre-mortem 5 và RK16 không phủ.
14. **Luật sư về Luật AI Đ.4(2) (R4).** Mặc định: câu trả lời là điều kiện bật chức năng tương ứng. *Đủ:* thiết kế đứng như viết. *Không đủ:* một dạng kiểm soát
    trước, thành một mã từ chối thêm hoặc rail người ký.
15. **Chữ ký trên tuyên bố rủi ro còn lại.** Mặc định: mỗi dòng RR một người cấp điều hành ký qua tin nhắn đã xác thực; không chữ ký thì chức năng không bật (một
    giới hạn lên quyết định triển khai của bên áp dụng, không lên agent). *Bỏ:* không ai chịu đuôi H2.
16. **Báo cáo hội đồng quản trị.** Mặc định: mỗi tháng, người cấp điều hành ký. *Mỗi quý:* chậm thấy hơn.
17. **Xác nhận R1–R10** (bảng từ ngữ). Một phán quyết bị bác thì các mục dẫn nó phải viết lại.

## 12. Hồ sơ hội đồng

Hội đồng: bốn bản chính `council/01-strategy.md`, `council/02-design.md`, `council/03-ledger.md`, `council/04-launch.md` và sáu phản biện
`council/challenge-01-evidence.md` đến `council/challenge-06-novelty.md` (nhãn [C01]–[C06]). Bốn phản biện đọc cho thiết kế của Remit, đã vào `remit-design.md`:
`council/design-review-01-security.md`, `council/design-review-01b-security.md`, `council/design-review-02-architecture.md`, `council/design-review-03-universality.md`.
Nguồn: `research-ai-autonomous-system-{evidence,design,risk,pilot}.md`, `brainstorm-ai-autonomous-system.md`, `papers/INDEX.md` (101 PDF); ADR trong `adr/`.

**Giả định:**
- Assumption: "doanh nghiệp lớn" có nhiều đơn vị kinh doanh, bộ phận tài chính đọc chi phí theo loại việc, hội đồng quản trị và pháp chế, vì mô hình ROI, nhóm so sánh và tuyên bố rủi ro còn lại cần các điều đó.
- Assumption: pháp nhân vận hành là công ty Việt Nam có khách ở Mỹ và EU, một phần là thể nhân kinh doanh, vì [C04] đọc luật trên cơ sở đó; bên áp dụng ở nơi khác thay mục 4.1 bằng luật nơi mình.
- Assumption: phán quyết R1–R10 và các mặc định ở mục 11 có hiệu lực cho bản này cho tới khi owner xác nhận hoặc bác, vì chủ tọa ghi chúng là mặc định.
- Assumption: nguồn pháp lý và thị trường chỉ đọc qua [C04], [C05], [C06] mang nhãn HIGH[Cxx] hoặc MEDIUM, vì hội đồng chưa mở lại văn bản gốc.
- Assumption: Remit là hiện thực tham chiếu của phương pháp, không phải điều kiện của nó; phương pháp chạy được trên nền tảng của bên áp dụng.
- Assumption: bên áp dụng có ít nhất một đơn vị kinh doanh ngoài ranh giới so sánh được, và mọi hệ thống trong ranh giới đi qua được gate và proxy, vì R10 cần đơn vị so sánh và F1, F2 chỉ đúng khi mọi tác dụng qua gate.
- Assumption: chữ ký người thu được bằng tin nhắn đã xác thực với Assistant, không cần biểu mẫu, vì `AGENTS.md:39–41` cấm biểu mẫu cấu hình.
