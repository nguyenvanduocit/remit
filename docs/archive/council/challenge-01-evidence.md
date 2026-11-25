# Phản biện 01: kiểm toán bằng chứng của bốn bản nháp hội đồng

> Thành viên hội đồng: kiểm toán bằng chứng (đối kháng nhưng công bằng). Ngày: 2026-11-01. Đối tượng:
> [`01-strategy.md`](01-strategy.md), [`02-design.md`](02-design.md), [`03-ledger.md`](03-ledger.md), [`04-launch.md`](04-launch.md).
> Bản này không sửa bản nháp nào; nó ghi điều sai, điều không đủ chứng cứ và cách sửa đúng từng chỗ.

## 0. Cách làm và giới hạn

- **PDF.** Trích văn bản cả 101 PDF trong `docs/papers/` bằng `pypdf` (venv tạm `/tmp/auditvenv`, đã xoá sau khi xong), 101/101
  tệp trích được, 0 lỗi. Mỗi con số tra bằng `rg -a` trong văn bản trích; kết quả ghi kèm cụm chữ gốc khi cần.
- **arXiv id.** 73 id riêng biệt xuất hiện trong bốn bản nháp; 72 có PDF trong kho, chỉ `2503.01747` (Bowyer) không có.
- **Web.** Mở lại 7 trang gốc: Devin annual review (cognition.com), Cursor background agents, Klarna press 27/02/2024, Zenity
  (PocketOS), OECD AI Incident Monitor (Replit), hai postmortem của Anthropic (`april-23-postmortem`,
  `a-postmortem-of-three-recent-issues`).
- **File nội bộ.** `AGENTS.md` trên đĩa (có khối *Our stance* ở dòng 19–29; số dòng bên dưới theo bản trên đĩa), ADR 0001, 0002,
  0010, 0013, 0016, 0018, `docs/design.md`, bốn báo cáo nghiên cứu.
- **Không kiểm được:** văn bản luật (Việt Nam, EU, Mỹ), điều khoản vendor, SEC, Vaccaro 2024, METR TH1.1 blog, Epoch/a16z, Project
  Vend 2, bài blog multi-agent của Anthropic, Sidley/Mayer Brown/Gibson Dunn. Các mục này ghi UNVERIFIED, không đoán.
- Kết quả: **CONFIRMED** (khớp nguồn), **WRONG** (sai; ghi giá trị đúng và chỗ), **UNSUPPORTED** (nguồn không nói),
  **OVERSTATED** (số đúng, cách nói vượt nguồn), **UNVERIFIED** (không có nguồn gốc trong tầm kiểm).

## 1. Sổ kiểm từng mục

Một dòng có thể gom nhiều con số của cùng một claim; cột **n** là số mục đã kiểm trong dòng đó.

### 1.1 `01-strategy.md`

| # | Vị trí | Claim | n | Kết quả | Bằng chứng |
|---|---|---|---|---|---|
| 1 | 01:45, 01:292 | Gần 9.000 lãnh đạo, 7 nước; năng suất +0,2%/năm; gần như không tác động ròng lên việc làm trong 3 năm | 4 | CONFIRMED | NBER w34836: "We survey nearly 9,000", "seven countries", "0.2% over the last 3 years", "essentially no net" |
| 2 | 01:47, 01:293 | 802 dev, 196.212 PR; +12% khi bắt đầu dùng AI; đạt gấp đôi ở tháng thứ mười; tải mỗi reviewer gấp đôi | 5 | CONFIRMED | 2607.01904 abstract: "only in the tenth month", "rose by about 12%", "load per reviewer doubled" |
| 3 | 01:43–48 | Dùng 2607.01904 làm bằng chứng "lợi ích cá nhân chưa thành lợi ích công ty" | 1 | OVERSTATED | Cùng abstract: công ty **đã đạt** mục tiêu 2× và tác giả gọi đó là "strong evidence that the adoption and sustained usage of AI raises PR throughput in an enterprise setting". Bài chỉ ủng hộ vế "chi phí dồn về review" |
| 4 | 01:97, 01:212 | "+12% PR mỗi người" như độ lớn hiệu ứng | 1 | OVERSTATED (theo chiều nhỏ đi) | +12% là bước nhảy lúc bắt đầu dùng; thông lượng "kept growing" tới 2× ở tháng 10 |
| 5 | 01:49, 01:294 | Cursor: tăng 3–5 lần tháng đầu, tan sau hai tháng; +30% cảnh báo, +41% độ phức tạp | 4 | CONFIRMED | 2511.04427: "3-5", "gains dissipate after two months", +30.26% (±6.66%), +41.64% (±7.62%). "Tốc độ" ở đây là số dòng thêm |
| 6 | 01:51, 01:295 | 52 MR chờ owner khi đo [ADR-0001] | 1 | **WRONG (trích dẫn)** | ADR-0001 không có số 52. Số nằm ở `ADR-0002:40`: "(52 waited when it was measured)" |
| 7 | 01:62, 01:296 | Độ dài tác vụ 50% gấp đôi khoảng 7 tháng | 1 | CONFIRMED | 2503.14499: "every seven months since 2019" |
| 8 | 01:63–64 | "khoảng 131 ngày", nhãn **HIGH** chung cho cả câu | 1 | UNVERIFIED; nhãn quá cao | Chỉ có ở blog METR TH1.1; DS §8 và 03 C10 ghi MEDIUM |
| 9 | 01:63, 01:297 | 15 model, 24 tháng; reliability 0,03/năm (GAIA), 0,09/năm (τ-bench) | 4 | CONFIRMED | 2602.16666 Hình 1: "slope=0.03/yr", "slope=0.09/yr"; "15 models", "24 months" |
| 10 | 01:64, 01:297 | "độ chính xác tăng 0,18 và 0,33" | 2 | **WRONG** | Độ chính xác tăng **0,23/năm (GAIA) và 0,22/năm (τ-bench)** (2602.16666 tr. 38, "slope=0.23/yr", "slope=0.22/yr"). 0,18 và 0,33 là độ dốc của reliability **theo accuracy** (Hình 1, trục x là Accuracy). PL:80 đã sửa đúng |
| 11 | 01:66, 01:298 | Giá ở năng lực cố định giảm 9–900 lần/năm, nhãn HIGH | 1 | UNVERIFIED; nhãn quá cao | DS §8 ghi HIGH nhưng DS §10 liệt Epoch, a16z vào "chưa đối chiếu toàn văn (MEDIUM)" |
| 12 | 01:68–69, 01:299–300 | Luật AI VN 01/03/2026, 01/03/2027, 18 tháng tài chính; PLD sau 09/12/2026 | 4 | UNVERIFIED | Khớp RK §1.2–1.3 (MEDIUM, MEDIUM-HIGH); văn bản luật không có trong kho |
| 13 | 01:76, 01:301 | Anthropic multi-agent +90,2%; 4× và 15× token | 3 | UNVERIFIED (khớp EV §3) | Blog không mở lại |
| 14 | 01:77, 01:302 | Magentic-One: task ledger, progress ledger | 1 | CONFIRMED | 2411.04468: "outer loop maintains the task ledger"; "Progress Ledger" |
| 15 | 01:78, 01:303 | MetaGPT: số lần người sửa 10 → 2,5 | 1 | CONFIRMED | 2308.00352 Bảng 3: hàng 1 role "$0.915 10", hàng 4 role "$1.385 2.5" |
| 16 | 01:80, 01:305 | Devin 67% PR merge so với 34%; "aren't straightforwardly verifiable" | 2 | CONFIRMED | cognition.com: "67% of its PRs are now merged vs 34% last year"; "When outcomes aren't straightforwardly verifiable, additional human review is necessary" |
| 17 | 01:81, 01:306 | Cursor cloud agent mở PR, chạy theo lịch hoặc sự kiện | 1 | CONFIRMED | cursor.com: "automatically open pull requests", "run them on a schedule or from events with Automations" |
| 18 | 01:82, 01:307 | Klarna 2/3 chat, +$40 triệu 2024 | 2 | CONFIRMED | Klarna press 27/02/2024 |
| 19 | 01:83 | "không ca A/B nào có lợi nhuận đã xác minh" | 1 | OVERSTATED (diễn đạt) | PL §3 chấm hạng A/B (đo độc lập / bên thứ ba), không phải A/B test; 01:285 viết đúng "A hoặc B" |
| 20 | 01:97, 01:310 | TheAgentCompany: khoảng 30%; kỹ thuật phần mềm cao nhất; hành chính, tài chính, DS thấp nhất | 3 | CONFIRMED | 2412.14161: 30.3%; "DS, Admin, and Finance tasks are the lowest" |
| 21 | 01:99, 01:314 | Knight > $460 triệu, khoảng 45 phút | 2 | UNVERIFIED (khớp EV §5, SEC) | Trang SEC không mở lại |
| 22 | 01:118 | "55,5% lần 'đạt' vi phạm quy trình" | 1 | **WRONG (mẫu số)** | 2610.25954: 55,5% là của **mọi** 840 lần chạy; trên lần đạt kết quả là **80,1%**. Mâu thuẫn với chính 01:315 |
| 23 | 01:118, 01:315 | τ-bench pass^8 < 25% bán lẻ | 1 | CONFIRMED | 2406.12045: "pass^8 < 25%" |
| 24 | 01:120, 01:311 | Ngang hàng 23,9 so với 67,7; kém khoảng 2,8 lần | 2 | CONFIRMED | 2512.08296 Bảng 5; "Decentralized to 23.9 (2.8× worse)" |
| 25 | 01:143, 01:317 | $1.126 trong 3,5 ngày, khoảng 114 chain, 8 MR, khoảng $141/MR | 5 | CONFIRMED, thiếu ngữ cảnh | `ADR-0002:25–26`. Thiếu: ADR-0002 đã bị ADR-0010 thay; hai ngày đo là dogfooding (`:46`); 83% chi tiêu là việc Scout tự giao (`:38–39`); ngày $479 có lẽ chỉ nửa ngày (`:37`) |
| 26 | 01:144, 01:147 | Remit nay không ghi chi phí, không đặt trần (ADR-0010) | 1 | CONFIRMED | `ADR-0010:23–27` |
| 27 | 01:153, 01:320 | 15 nền tảng; "không gì thay được thí nghiệm dài hạn" | 2 | CONFIRMED | 2608.08043: "26 experts from 15 online platforms", "no substitute for a well-run long-term experiment" |
| 28 | 01:183 | Model tốt nhất sai khoảng 1 trên 6 câu | 1 | CONFIRMED | 2603.07316: "roughly 1 in 6 questions is wrong" |
| 29 | 01:194–195, 01:312 | +80,8% (tập trung); lập kế hoạch tuần tự tới −70% | 2 | CONFIRMED | 2512.08296 abstract |
| 30 | 01:227–228, 01:335 | Tập neo phát hiện 60/60 | 1 | CONFIRMED | 2606.15474: "detected as judge drift in 60/60 runs" |
| 31 | 01:231, 01:336 | Đổi judge làm 59% tập top-30% đổi | 1 | CONFIRMED | 2610.32986 |
| 32 | 01:231, 01:337 | "nhà cung cấp ngừng model sau tối thiểu 60 ngày" | 1 | OVERSTATED | PL §4 chỉ nói về Anthropic (trang deprecations, MEDIUM) |
| 33 | 01:246, 01:328 | Replit: "nói khôi phục là bất khả trong khi rollback vẫn dùng được" | 1 | **UNSUPPORTED hiện nay** | Trang OECD (mở 2026-11-01) ghi "permanent data loss"; 04:271–272 đã nêu nguồn mâu thuẫn |
| 34 | 01:247, 01:329 | PocketOS: token trong tệp không liên quan, 9 giây, backup cùng volume | 3 | CONFIRMED | Zenity: "found one in an unrelated file", "The deletion took 9 seconds", "backups in the same volume" |
| 35 | 01:248, 01:330 | Palisade > 100.000 lượt, 13 model, tới 97% dù được dặn | 3 | CONFIRMED | 2509.14260: "up to 97% (95% CI: 96-98%)" |
| 36 | 01:252, 01:331 | 68% tối đa 10 bước; 74% dựa vào đánh giá người | 2 | CONFIRMED | 2512.04123 |
| 37 | 01:253, 01:333 | 94% (83/88) không monitor không phát hiện | 1 | CONFIRMED | 2606.05647 |
| 38 | 01:255–256, 01:334 | Vaccaro g = −0,23, +0,46, +0,19 | 3 | UNVERIFIED (khớp RK §2.1) | Bài không có trong kho |
| 39 | 01:276, 01:326 | Bing: truy vấn > 10%, doanh thu mỗi người > 30% | 2 | CONFIRMED | Kohavi 2012: "went up over 10%", "over 30%" |
| 40 | 01:286, 01:309 | CEO-Bench: mọi model thua baseline theo quy tắc | 1 | CONFIRMED | 2606.18543: "all evaluated models remain below the rule-based baseline" |
| 41 | 01:322 | 9 judge ≈ 2 phiếu; ba judge frontier ρ = 0,56 | 2 | CONFIRMED | 2605.29800 (n_eff 2,18); 2609.22512 |
| 42 | 01:324, 01:180 | Phạt tới 5% doanh thu; hồ sơ 60 ngày (Luật 91/2025) | 2 | UNVERIFIED (khớp RK §1.3 HIGH) | Văn bản luật không có trong kho |
| 43 | 01:339 | Trần trách nhiệm Anthropic = phí 12 tháng | 1 | UNVERIFIED (khớp RK §1.4) | Điều khoản không mở lại |

### 1.2 `02-design.md`

| # | Vị trí | Claim | n | Kết quả | Bằng chứng |
|---|---|---|---|---|---|
| 44 | 02:30 | Knight > $460 triệu, 45 phút | 1 | UNVERIFIED (khớp EV §5) | — |
| 45 | 02:36–38 | 30% TheAgentCompany; 2,5% RLI; CEO-Bench | 3 | CONFIRMED | 2412.14161; 2510.26787 ("automation rate of 2.5%"); 2606.18543 |
| 46 | 02:56 | I7: "MSR 2605.22534: 33,1% không lý do" | 1 | OVERSTATED (thiếu mẫu số) | 33,1% của 353 PR bị từ chối trong mẫu 717 PR đọc tay |
| 47 | 02:118 | "31,2% PR bị đóng trong MSR là do quy trình" | 1 | **WRONG (mẫu số)** | 2605.22534 Bảng 2 (N = 353): 31,2% **của PR bị từ chối** trong mẫu phân tầng 717, không phải của 11.048 PR đóng |
| 48 | 02:57, 02:281 | Song: nhiều vòng thêm 62% báo động nhầm | 1 | CONFIRMED | 2603.16244: "62% more false positives (8.47 vs. 5.23)" |
| 49 | 02:90 | g = −0,23; ra quyết định −0,27; +0,46 | 3 | UNVERIFIED (khớp RK §2.1) | — |
| 50 | 02:94 | Goal judge không thuộc đội | 1 | CONFIRMED | `docs/design.md:433` ("It is not on the Chair's team") |
| 51 | 02:98–99 | 0,477 so với 0,466; 23,9 so với 67,7; tuần tự −39 đến −70%; trần 45% | 4 | CONFIRMED | 2512.08296: Bảng 5; "from -70% to -39%"; "β=−0.236, p=0.004 ... exceeds 45%" |
| 52 | 02:99 | Ghi nguồn "(Kim 2512.08296 Bảng 5)" cho cả câu | 1 | OVERSTATED (trích dẫn) | Trần 45% và −39–70% nằm ở phần hồi quy và theo benchmark, không ở Bảng 5 |
| 53 | 02:131, 02:368 | Xung đột 41,7% khác agent so với 19,8% cùng agent | 2 | CONFIRMED số, thiếu đơn vị | 2607.04697: cặp PR, 48/115 (KTC 33,1–50,9%) và 119/601 |
| 54 | 02:132 | −39% khi nhiều lượt | 1 | CONFIRMED | 2505.06120 |
| 55 | 02:134 | Bộ nhớ tự tiến hoá: từ chối 99,4% → 54,4% | 1 | CONFIRMED | 2509.26354 |
| 56 | 02:147–148, 02:59 | FinSheet 82,4%; không model nào đủ an toàn không giám sát | 2 | CONFIRMED | 2603.07316: "None of the ten configurations tested produce error rates acceptable for unsupervised production" |
| 57 | 02:151–152 | ρ = 0,56; cùng sai 7,7×; 9 judge ≈ 2 phiếu | 3 | CONFIRMED | 2609.22512; 2605.29800 |
| 58 | 02:153 | Reweighting +3,5 điểm | 1 | CONFIRMED | 2604.07650: "3.5 and 2.6 percentage-point gains" (verifier tự động từng bác sai) |
| 59 | 02:154 | Đo κ, không tỷ lệ khớp thô (2606.19544) | 1 | CONFIRMED | 2606.19544: thổi phồng "33–41 pp". Hoán đổi A/B là của Zheng et al. (PL §5.1), không phải 2606.19544 |
| 60 | 02:156 | Generator càng mạnh càng khó kiểm (2509.17995) | 1 | CONFIRMED | "As generators become stronger" |
| 61 | 02:159 | Tập neo 60/60; z-test báo nhầm 75%; 59% top-30% | 3 | CONFIRMED | 2606.15474: "false-alarms on 75% of drift-free streams"; 2610.32986 |
| 62 | 02:161 | CXBench 55,5% lần chạy là false-pass | 1 | CONFIRMED | 2610.25954 |
| 63 | 02:176, 03:295 | Sigerson: surrogate "bị khai thác bằng thông báo mỗi 7 ngày" | 1 | **OVERSTATED** | 2608.08043 nêu như ví dụ giả định: "one might find a correlation ... then try to move that short-term outcome ... (for example, by sending notifications every 7 days)". Không phải ca đã quan sát |
| 64 | 02:180 | Netflix recall 65% "ngay cả khi có 200 A/B test" | 1 | OVERSTATED | 200 A/B test (1.098 arm) là mẫu đánh giá, không phải điều kiện. Bài tự mâu thuẫn: abstract "79% and 65% recall", thân bài "79% precision and 65% recall" |
| 65 | 02:180, 02:364 | So trước/sau bị thời gian làm nhiễu (SRE Workbook) | 1 | UNVERIFIED (khớp EV §5) | — |
| 66 | 02:184 | Bing (Kohavi 2012) | 1 | CONFIRMED | xem #39 |
| 67 | 02:186 | Monitor vào reward thì bị né (Baker) | 1 | CONFIRMED | 2503.11926 |
| 68 | 02:208 | MCPTox 72,8% ASR | 1 | CONFIRMED | 2508.14925 |
| 69 | 02:209–211 | > 90% vượt 12 phòng thủ; 58–90%; CaMeL 77% so với 84% | 4 | CONFIRMED | 2510.09023; 2503.12188; 2503.18813 |
| 70 | 02:223 | "Lỗi định tuyến tới 16% yêu cầu" | 1 | CONFIRMED, thiếu phạm vi | Postmortem: "At the worst impacted hour on August 31, 16% of Sonnet 4 requests" |
| 71 | 02:221 | Anthropic: ba thay đổi harness, eval không tái hiện | 1 | CONFIRMED | april-23-postmortem: "neither our internal usage nor evals initially reproduced the issues" |
| 72 | 02:231 | "94% lập trình viên không phát hiện phá hoại" | 1 | OVERSTATED | 94% là nhóm **không có monitor** (83/88); 04:93 viết đúng |
| 73 | 02:232 | Monitor báo đúng vẫn 56% thành công | 1 | CONFIRMED | 2606.05647: "9/16 (56%)" |
| 74 | 02:232–233 | 86% (Anthropic 2026-10-27); 93% phê duyệt; nhãn HIGH cho cả khối | 2 | UNVERIFIED; nhãn quá cao | 93% là MEDIUM-HIGH ở RK §2.1 |
| 75 | 02:237 | Kale: TPR +15% ở FPR 0,01 | 1 | CONFIRMED | 2508.19461: "approximately 15% at FPR = 0.01" |
| 76 | 02:238–239 | Vasconcelos: kiểm được trong vài giây | 1 | CONFIRMED | 2212.06823 (N = 731) |
| 77 | 02:241–242 | Outcomes đã đo tỷ lệ merge không bình luận | 1 | CONFIRMED | `docs/design.md` mục Outcomes: "the share of merged chains he merged without a review comment" |
| 78 | 02:272 | "đặc tả là 41,77% lỗi MAST v2" | 1 | UNVERIFIED trong kho; sửa được | Kho có v3. v3 Hình 4 (210 trace): **System Design Issues 41,8%, Inter-Agent Misalignment 36,9%, Task Verification 21,3%**. Nhóm tên "thiết kế hệ thống", không chỉ "đặc tả" |
| 79 | 02:279 | Vend 2: CEO agent duyệt gấp khoảng 8 lần số từ chối | 1 | UNVERIFIED, thiếu nhãn | DS §8 ghi MEDIUM; 02 không ghi nhãn |
| 80 | 02:280 | McAleese: reviewer bịa lỗi | 1 | CONFIRMED | 2407.00215 (63%, "hallucinate") |
| 81 | 02:268 | Chain đóng khi **auditor** xác nhận (ADR-0013) | 1 | OVERSTATED | `ADR-0013:33–35`: goal judge ra phán quyết, không phải auditor |
| 82 | 02:388 | 70% hệ thống thật dùng model có sẵn với prompt | 1 | CONFIRMED | 2512.04123: "70% rely on prompting off-the-shelf" |
| 83 | 02:426 | DGCL §141(b) giám đốc là thể nhân | 1 | UNVERIFIED (khớp PL §3 MEDIUM) | — |
| 84 | 02:436 | "93% phê duyệt" áp cho owner merge | 1 | OVERSTATED | 93% là lời nhắc xin quyền của Claude Code (RK §2.1), không phải duyệt merge mã |
| 85 | 02:26–27 | F4: rail gồm "xác thực, pause và stop, merge, credential" | 1 | OVERSTATED | `AGENTS.md:47–48` nêu ba rail; credential (`:49–51`) là trường chỉ ghi, không gọi là rail |
| 86 | 02:51, 02:195 | ADR-0018 là hiện thực tham chiếu; F2 "mọi agent dùng được mọi connection" | 1 | **Mâu thuẫn với ADR** | `ADR-0018:40–43` (Least grant: grant `write` hoặc connection protected "waits for the owner") và `:98`. Bản nháp giữ 2 trong 4 invariant |

### 1.3 `03-ledger.md` (các ô RECHECKED được tra lại độc lập)

| # | Vị trí | Claim | n | Kết quả | Bằng chứng |
|---|---|---|---|---|---|
| 87 | 03:47–55 | C01–C09 (30,3%/39,3%/$4,2/26,3%/$4,1; 2,5%/240/$143.991/$1.720; 47,6% Opus 4.1; 46,8%/$3,79/$3,78/3,1 phút/16,8 phút/$25,66; 82,4/48,6/86,2; $15,8M/$12,6M/$1M/500 ngày; 55,5/80,1/13,8/840; 61/35/25/34%; 25/34%/120) | 35 | CONFIRMED | 2412.14161, 2510.26787, 2510.04374, 2508.00828, 2603.07316, 2606.18543, 2610.25954, 2406.12045, 2506.07982, 2607.22928 |
| 88 | 03:56 | C10 7 tháng; 131 ngày MEDIUM | 2 | CONFIRMED / UNVERIFIED | như #7, #8 |
| 89 | 03:62 | C11 "accuracy 0,18 và 0,33", ghi RECHECKED | 2 | **WRONG** | như #10. Lỗi nằm trong chính sổ bằng chứng và mang nhãn đã kiểm |
| 90 | 03:62 | C11 p = 0,08 (GAIA); nhất quán τ-bench 0,52–0,90 | 2 | CONFIRMED (0,08; 0,52); 0,90 không tra | 2602.16666 |
| 91 | 03:63–67 | C12–C16 (60.000, 2,2–6,0, > 1,5, Atil 15%/70%/10 lần; −39/+112/−15/−16; 93%/59%; 60/60, 75%) | 15 | CONFIRMED | 2602.07150, 2408.04667 ("gap ... up to 70%"), 2505.06120, 2610.32986, 2606.15474 |
| 92 | 03:65 | C14 ghi "chưa (không phải PDF)" | 1 | CONFIRMED (web, lần này) | Postmortem: "initially affected 0.8%", "16% of Sonnet 4 requests", "simply didn't capture the degradation" |
| 93 | 03:73–81 | C17–C25 (260, +80,8/−70,0; Bảng 5; β, 22,7% [20,1; 25,3]; +14%/+44%/d 1,86/25.000/95%/24×; 51,5%/d 0,42/0,34/53%/−0,14; F1 0,376/0,303, 8,5/5,2, 0,286/0,667, 0,457/0,495; 717/353; 107.026/29.609; 48/115, 119/601) | 34 | CONFIRMED | 2512.08296, 2603.28990, 2609.14767, 2603.16244, 2608.18167, 2605.22534, 2604.03551, 2607.04697 |
| 94 | 03:82 | C26 "Không kiểm được: v3 không có các số này" | 1 | **WRONG** | v3 Hình 4: 41,8% / 36,9% / 21,3% trên 210 trace (làm tròn của 41,77 / 36,94 / 21,30), nhóm đổi tên thành System Design Issues |
| 95 | 03:83 | C27 blackboard +13–57% | 1 | CONFIRMED; "1,8–2,3 lần" không tìm thấy bằng grep | 2510.01285 |
| 96 | 03:89–94 | C28–C33 (60%, > 350; 2,18 [2,07; 2,31], 1,93, 11% kể cả oracle; φ 0,437/0,435/0,389/0,047/0,603/0,588/0,564; 0,206/3,51/1.133/91–93%/0,56/7,7; 18,5%, +3,5/+2,6; FPR 80%) | 25 | CONFIRMED | 2506.07962, 2605.29800 §5.3, 2609.22512, 2609.17857, 2604.07650, 2507.08794 |
| 97 | 03:100–104 | C34–C38 (83/88, 12/19, 16/19, 9/16; AUC 0,65/0,87, 103 phút; 89%; 0,49/0,08/0,27, N 199; TPR +15%; 41 chính sách) | 14 | CONFIRMED | 2606.05647, 2506.15740, 2411.11401 (kể cả "did not help in identifying more high-severity issues"), 2102.09692, 2508.19461, 2109.05067 |
| 98 | 03:105–107 | C39–C41 (86%, 60%, 77%, 15%; Vaccaro; 93%) | 3 | UNVERIFIED (đúng như sổ tự ghi "chưa") | — |
| 99 | 03:113–123 | C42–C52 (AgentDojo 47,69/33,86/11,29, 57,69 → 6,84/7,95, 69,0 → 41,49; > 90%/12; 45/353/1.348/72,8/< 3%; 58–90, 66, 76; 1,8 triệu/60.000/22/44, 86%, 0–17%; 77/84; 97% [96; 98]; > 85%, 0,99 → 0,34, 13 → 0,4, 8,7 → 0,3; 95/60; 12%, 65%; 99,4 → 54,4, 0,6 → 20,6, 18.282 → 122, 66,7/57,1/63,7, 0,870 → 0,606) | 40 | CONFIRMED | 2406.13352 (văn bản ghi 7,5% cho lọc công cụ), 2510.09023, 2508.14925, 2503.12188, 2410.07283, 2507.20526, 2504.18575, 2503.18813, 2509.14260, 2412.04984, 2509.15541, 2503.11926, 2511.18397, 2509.26354, 2510.04618, 2410.10813 |
| 100 | 03:124 | C53 Replit, PocketOS | 2 | PocketOS CONFIRMED (web); Replit khôi phục: nguồn mâu thuẫn | xem #33, #34 |
| 101 | 03:130–133 | C54, C55, C57 (> 10%/> 30%; Google −50% quảng cáo mobile; Netflix 95%/65%; Athey −35% SE; $2.217,93/$476,00/$844,05) | 8 | CONFIRMED | Kohavi 2012, Hohnhold 2015, 2311.11922, Athey NBER w26463, 2502.15840 |
| 102 | 03:139–142 | C58–C61 (+30,26/+41,64; 802/196.212; 306/86/68/70/74%; +0,2/+0,6, 19%, 22–25 tuổi) | 13 | CONFIRMED | 2511.04427, 2607.01904, 2512.04123, NBER w34836, Brynjolfsson |
| 103 | 03:149 | C63 dự thảo 36 trang, "likely to be introduced ... next year" | 2 | CONFIRMED | Thư House Oversight 2026-10-26 |
| 104 | 03:26 | "54 hàng RECHECKED" | 1 | CONFIRMED | `rg '\| RECHECKED'` đếm 54 |
| 105 | 03:26 | "bốn con số cần thêm sắc thái (mục 1.10)" | 1 | WRONG (đếm) | Bảng 1.10 có **5** dòng sắc thái (C18, C29, C55, C13, C56) cộng 3 dòng đổi mẫu số |
| 106 | 03:34 | Assumption "Palisade, C53" | 1 | **WRONG (nhãn)** | Palisade là **C48** (03:119); C53 là Replit/PocketOS |
| 107 | 03:166 | "Mọi con số RECHECKED khác khớp báo cáo" | 1 | WRONG | C11 mang RECHECKED mà sai (#89) |

### 1.4 `04-launch.md`

| # | Vị trí | Claim | n | Kết quả | Bằng chứng |
|---|---|---|---|---|---|
| 108 | 04:19–20 | Trích `AGENTS.md:21-22` | 1 | CONFIRMED nguyên văn | `AGENTS.md:21–22` |
| 109 | 04:30, 04:31, 04:33 | `AGENTS.md:47-49` (pause and stop; "no agent lifts or moves a rail (merges, signs in)"), `:49-51` (credential) | 3 | CONFIRMED | `AGENTS.md:47–51` |
| 110 | 04:59, 04:90, 04:95, 04:107, 04:116, 04:146 | `AGENTS.md:43-44`, `:53`, `:45-46`, `:45`, `:15-16` | 6 | CONFIRMED | Đúng dòng ở bản trên đĩa |
| 111 | 04:55, 04:275–277 | Palisade 97% [96; 98]; 8/13 khi không chỉ dẫn; 6/13 khi thêm "allow yourself to be shut down"; 1.000 lượt | 5 | CONFIRMED | 2509.14260: "8/13 models sabotaged the shutdown script at least once in 1000 trials", "dropped to 6/13" |
| 112 | 04:56 | PocketOS 25/04, `volumeDelete`, 9 giây | 3 | CONFIRMED | Zenity: "On April 25th", "including destructive operations like volumeDelete" |
| 113 | 04:58 | Apollo: o1 > 85% | 1 | CONFIRMED | 2412.04984 |
| 114 | 04:61, 04:292–295 | Harness: 04/03 hạ mức suy luận, 26/03 xoá suy luận cũ, 16/04 dòng prompt; "soak periods, a broader eval suite, and gradual rollouts" | 4 | CONFIRMED | april-23-postmortem |
| 115 | 04:77, 04:81 | "simply didn't capture ..."; "continuously on true production systems"; 0,8% và 16% ngày 31/8 | 4 | CONFIRMED | a-postmortem-of-three-recent-issues |
| 116 | 04:84 | z-test "báo động nhầm **trên** 75% chuỗi" | 1 | OVERSTATED (nhẹ) | 2606.15474: "75% of drift-free streams" |
| 117 | 04:93 | Tufano 89%; 94% không monitor | 2 | CONFIRMED | 2411.11401; 2606.05647 |
| 118 | 04:119 | Một lần chạy dao động 2,2–6,0 điểm ở temperature 0 | 1 | CONFIRMED | 2602.07150 |
| 119 | 04:128 | +30,26%, +41,64%, "dissipate after two months" | 3 | CONFIRMED | 2511.04427 |
| 120 | 04:141 | M1: pass^k, C_out = trung bình (2p̂ − 1)², Rabanser Bảng 2; ví dụ 0,40 và 0,36 | 3 | CONFIRMED | 2602.16666 Bảng 2 "Reliability metrics overview"; C(4,3)/C(5,3) = 0,40; (1,6 − 1)² = 0,36 |
| 121 | 04:142 | CXBench 55,5/80,1/13,8, 7 model, 120 tác vụ | 5 | CONFIRMED | 2610.25954 |
| 122 | 04:145, 04:244–245 | Pan 68% của 86 hệ thống, 74%; tải reviewer khoảng 2,0 lần | 4 | CONFIRMED | 2512.04123; 2607.01904 ("about twice as many PRs") |
| 123 | 04:151 | Vending $2.217,93, $476,00, 21% | 3 | CONFIRMED | 2502.15840 Bảng 1; 476/2217,93 = 0,215 |
| 124 | 04:152–153 | Bowyer (không có PDF, MEDIUM); Miller §2.2 sai số theo cụm | 2 | UNVERIFIED / CONFIRMED | 2411.00640: "2.2 Clustered questions" |
| 125 | 04:183 | o3 76% so với 53% | 2 | CONFIRMED | 2506.12286 |
| 126 | 04:206 | TheAgentCompany 30%, thứ hạng role | 2 | CONFIRMED | xem #20 |
| 127 | 04:242 | Klarna 27/02/2024, 2,3 triệu chat, 2/3, 700 nhân viên, +$40 triệu | 5 | CONFIRMED | Klarna press |
| 128 | 04:270–272 | Replit: OECD ghi mất dữ liệu vĩnh viễn, nguồn mâu thuẫn | 1 | CONFIRMED | OECD: "permanent data loss"; khoảng 4.000 hồ sơ chỉ có ở tiêu đề bài dẫn |
| 129 | 04:279–281 | Rabanser 0,03/0,09 và đính chính 0,18/0,33 | 3 | CONFIRMED; ghi chú đã cũ | PL:80 đã đọc 0,23/0,22, nên "đính chính cho báo cáo pilot (dòng 80)" không còn đúng |
| 130 | 04:282, 04:299 | Mọi model Vending-Bench có lần "derail"; 59% | 2 | CONFIRMED | 2502.15840: "all models have runs that derail"; 2610.32986 |
| 131 | 04:227–233 | EU Omnibus (Gibson Dunn 06/05/2026; Annex III 02/12/2027); ISO/IEC 42001 18/12/2023; Luật AI VN | 4 | UNVERIFIED | Không có văn bản trong tầm kiểm |

## 2. Tham chiếu `AGENTS.md` và ADR

| Tham chiếu | Ở đâu | Kết quả |
|---|---|---|
| `AGENTS.md` ba rail: xác thực, pause và stop, merge | 01:28, 03:36, 04:35 | CONFIRMED `AGENTS.md:47–48` |
| `AGENTS.md` rail gồm cả "credential" | 02:26 | OVERSTATED: credential ở `:49–51` là trường chỉ ghi, không gọi là rail |
| `AGENTS.md:15–16, 21–22, 43–44, 45, 45–46, 47–49, 49–51, 53` | 04 | CONFIRMED cả 8, theo bản trên đĩa (có khối *Our stance*) |
| ADR-0001 "52 MR chờ owner" | 01:51, 01:295 | **WRONG**: ADR-0001 chỉ có cap N = 5 (`:20`); số 52 ở `ADR-0002:40` |
| ADR-0001 còn giới hạn số MR chờ | 01:28–29 | CONFIRMED `ADR-0001:20, 28`; `ADR-0010:31` giữ `-max-waiting-mrs`. 01 là bản duy nhất nói quyết định bỏ cap này; 02, 04 im lặng |
| ADR-0002 $1.126, 3,5 ngày, khoảng 114 chain, 8 MR, $141 | 01:143 | CONFIRMED `ADR-0002:25–26`; ADR-0002 trạng thái "superseded by 0010" (`:3`) |
| ADR-0010 không ghi chi phí, không trần | 01:144, 01:147, 04:146 | CONFIRMED `ADR-0010:23–27` |
| ADR-0013 chain giữ kết quả đến khi hiệu ứng được kiểm chứng | 01:76, 01:176, 02:268 | CONFIRMED; nhưng 02:268 gán việc xác nhận cho "auditor", ADR ghi goal judge (`:33–35`) |
| ADR-0016 khách hàng là ai và nhận gì | 02:104 | CONFIRMED `ADR-0016:30–34` |
| ADR-0018 "Sealed, One gate" | 01:25, 02:51, 02:195 | CONFIRMED `ADR-0018:38–46`. Nhưng `:40–43` (Least grant) bị F2 của 02 bỏ, và grant `write` "waits for the owner" là một bước duyệt, trái "không cổng duyệt". ADR-0018 không có dòng `Status:` |
| `docs/design.md` goal judge, Outcomes | 02:94–95, 02:241 | CONFIRMED (`design.md:433`, mục Outcomes) |

## 3. Mâu thuẫn giữa các bản nháp

| # | Chủ đề | Bản A nói | Bản B nói | Mức |
|---|---|---|---|---|
| X1 | Rail merge và số điểm cố định | 01:21–29: ba điểm cố định; quyết định **không giữ merge** và cap MR | 02:24–27, 02:197: F4 giữ merge do owner; 04:33–39: giữ merge theo `AGENTS.md`; 03:36–37: nêu xung đột, không chọn | MAJOR |
| X2 | Lặp nhiều lần và pass^k | 03:282 (R11): "không có lặp nhiều lần hay pass^k vì đó là thí nghiệm" | 02:162–163, 02:222: K ≥ 3, pass^k; 04:61, 04:119, 04:141: K ≥ 5, pass^k ở M1 | MAJOR |
| X3 | Số lần lặp K | 02: K ≥ 3 | 04: K ≥ 5 | MINOR |
| X4 | Replit khôi phục | 01:246: "rollback vẫn dùng được" | 04:271–272: nguồn mâu thuẫn, OECD ghi mất vĩnh viễn | MAJOR |
| X5 | CXBench | 01:118: 55,5% của lần đạt | 01:315, 02:161, 03:53, 04:142: 55,5% của mọi lần chạy, 80,1% của lần đạt | BLOCKER (trong 01) |
| X6 | Rabanser accuracy | 01:64, 01:297, 03:62: 0,18 và 0,33 | 04:280–281, PL:80: 0,23 và 0,22; 0,18/0,33 là theo accuracy | BLOCKER |
| X7 | "Không bí mật" và lỗi gieo | 02:420–421: nhãn lỗi gieo ẩn với agent bị kiểm | 03:33: "every business fact is open to every agent" | MAJOR |
| X8 | Phạm vi connection | 02:195: mọi agent dùng được mọi connection | ADR-0018:40–43 (least grant), mà 02:51 gọi là hiện thực tham chiếu | MAJOR |
| X9 | Canary suy giảm | 03:297: "không có canary suy giảm (thí nghiệm)" | 02:183–184: thử thước đo trên dữ liệu phát lại, giữ như kiểm tra dụng cụ | MINOR |
| X10 | Mức dừng | 02:194: toàn bộ, cell, tenant, agent | 04:212–216: agent, ranh giới, nhà cung cấp model | MINOR |
| X11 | Nguồn hạn EU AI Act | 03:151, 03:337: Reg. (EU) 2026/1744, hiệu lực 27/07/2026 (Mayer Brown, Sidley 22/06) | 04:228–230: thoả thuận chính trị 06/05/2026 (Gibson Dunn) | MINOR, chưa kiểm được bên nào |
| — | Lịch đọc số | 01: từ tháng thứ ba; 02: tối thiểu 8–12 tuần; 04: tuần 8 và 12 | Nhất quán | — |

## 4. Đúng nhưng nói vượt nguồn

1. **2607.01904 làm bằng chứng cho "bài toán"** (01:43–48, 01:97, 01:212): bài cho thấy công ty đạt 2× ở tháng 10 và AI tăng thông lượng;
   chỉ vế "chi phí dồn về review" ủng hộ luận điểm. "+12%" là bước nhảy lúc bắt đầu dùng, không phải hiệu ứng cuối.
2. **Sigerson "bị khai thác"** (02:176, 03:295): ví dụ giả định của tác giả, không phải ca đã xảy ra.
3. **Netflix** (02:180): "ngay cả khi có 200 A/B test" biến cỡ mẫu thành điều kiện; bài tự mâu thuẫn precision/recall (03 C55 đã ghi).
4. **94%** (02:231): thiếu "khi không có monitor".
5. **93% phê duyệt** (02:233, 02:436): đo lời nhắc xin quyền của Claude Code, không đo owner duyệt merge.
6. **MSR 31,2% và 33,1%** (02:56, 02:118): phải ghi "của 353 PR bị từ chối trong mẫu 717 PR đọc tay".
7. **41,7% và 19,8%** (02:131, 02:368): phải ghi đơn vị cặp PR và n = 115 ở nhánh khác agent.
8. **60 ngày báo trước** (01:231): chỉ Anthropic, MEDIUM.
9. **Nhãn HIGH quá rộng**: 01:64 phủ cả 131 ngày (MEDIUM); 01:66 Epoch/a16z (nguồn tự mâu thuẫn HIGH/MEDIUM); 02:233 phủ 93% (MEDIUM-HIGH).
10. **$141 mỗi MR** (01:143): thiếu ngữ cảnh dogfooding, 83% chi do Scout tự giao (Scout nay đã bỏ, ADR-0023), một ngày đo là nửa ngày.
11. **"Trên 75%"** (04:84): nguồn ghi 75%.
12. **"(Kim Bảng 5)"** (02:99): trần 45% và −39–70% không ở Bảng 5.
13. **"Lỗi định tuyến tới 16% yêu cầu"** (02:223): 16% yêu cầu Sonnet 4 trong giờ tệ nhất ngày 31/8.
14. **"Auditor xác nhận"** (02:268) thay cho goal judge của ADR-0013.
15. **"không ca A/B nào"** (01:83): đọc như A/B test; nguồn là hạng A/B.

## 5. Bảng mức độ và cách sửa

| Mức | Mục | Sửa chính xác |
|---|---|---|
| **BLOCKER** | Rabanser: 01:64, 01:297, 03:62 (C11, đang ghi RECHECKED) | Thay bằng: "độ tin cậy tăng 0,03/năm (GAIA) và 0,09/năm (τ-bench), độ chính xác tăng 0,23/năm và 0,22/năm; mỗi đơn vị độ chính xác chỉ kéo độ tin cậy lên 0,18 và 0,33" (2602.16666 Hình 1 và tr. 38). Ở 03: sửa C11 và 03:166 |
| **BLOCKER** | CXBench: 01:118 | "80,1% số lần đạt kết quả vi phạm quy trình (55,5% của mọi 840 lần chạy)" (2610.25954) |
| MAJOR | 52 MR: 01:51, 01:295 | Đổi nguồn thành `ADR-0002:40` |
| MAJOR | MSR: 02:118 (và 02:56) | "31,2% PR bị từ chối trong mẫu 717 PR đọc tay (353 bị từ chối) là do quy trình" |
| MAJOR | 2607.01904 trong luận điểm: 01:43–48, 01:97, 01:212 | Ghi đủ: thông lượng +12% lúc bắt đầu, đạt 2× ở tháng 10, chi phí dồn về review; bỏ cách đọc "lợi ích không lên cấp công ty" khỏi bài này |
| MAJOR | X1 rail merge | Hội đồng chọn một: hoặc owner sửa `AGENTS.md:47–49` và ADR-0001, hoặc 01 giữ merge như 02, 04. Không để bốn bản nói hai điều |
| MAJOR | X2 pass^k | Sửa 03:282 (R11): lặp eval trên tác vụ ghi lại là kiểm tra dụng cụ, không phải thí nghiệm trên khách (như 02:163); chọn một K |
| MAJOR | X4 Replit: 01:246, 01:328 | "Agent nói khôi phục là bất khả; các nguồn mâu thuẫn về việc rollback có dùng được không (OECD hiện ghi mất vĩnh viễn)" |
| MAJOR | X7 lỗi gieo và "không bí mật" | Ghi một định nghĩa chung: nhãn lỗi gieo ẩn với agent bị kiểm tới khi ca được giải quyết, hiện với owner; sửa assumption 03:33 |
| MAJOR | X8 ADR-0018: 02:195, 02:51 | Hoặc ghi rõ thiết kế bỏ invariant 2 của ADR-0018 và đề xuất ADR thay thế, hoặc giữ least grant; không gọi ADR-0018 là hiện thực tham chiếu khi bỏ một nửa nó |
| MINOR | Sigerson: 02:176, 03:295 | "có thể bị khai thác, ví dụ tác giả nêu: gửi thông báo mỗi 7 ngày" |
| MINOR | Netflix: 02:180 | "trên 200 A/B test (1.098 arm), recall 65% so với quyết định phát hành thật" |
| MINOR | 94%: 02:231 | "94% (83/88) lập trình viên không có monitor" |
| MINOR | 93%: 02:436 | Bỏ hoặc ghi "93% lời nhắc xin quyền của Claude Code được chấp nhận (MEDIUM-HIGH), suy rộng sang merge" |
| MINOR | MAST: 02:272, 03:82 | "System design 41,8% (MAST v3, Hình 4, 210 trace)"; sửa C26 thành RECHECKED trên v3 |
| MINOR | 41,7%/19,8%: 02:131, 02:368 | "48/115 cặp PR khác agent so với 119/601 cặp cùng agent" |
| MINOR | Nhãn: 01:64, 01:66, 02:233, 02:279 | 131 ngày MEDIUM; Epoch MEDIUM; 93% MEDIUM-HIGH; Vend 2 MEDIUM |
| MINOR | 03:34 | "Palisade, C48" |
| MINOR | 03:26 | "năm con số cần thêm sắc thái" |
| MINOR | 04:84 | "75% chuỗi không có thay đổi" |
| MINOR | 04:279–281 | Bỏ "đính chính cho báo cáo pilot (dòng 80)"; PL:80 đã đúng |
| MINOR | 01:231 | "Anthropic báo trước tối thiểu 60 ngày (MEDIUM)" |
| MINOR | 02:268 | "đóng khi goal judge xác nhận (ADR-0013)" |
| MINOR | 02:26 | Rail theo `AGENTS.md:47–48` là ba; credential là trường chỉ ghi `:49–51` |
| MINOR | 02:99 | Nguồn "2512.08296 (Bảng 5; hồi quy trần năng lực)" |
| MINOR | 01:143 | Thêm "dogfooding; 83% chi do Scout; ADR-0002 đã bị ADR-0010 thay" |
| MINOR | 01:83 | "không ca hạng A hoặc B nào" |

## 6. Đếm trung thực: đã kiểm và chưa kiểm

- **Tổng thể.** Bốn bản nháp có khoảng 541 token số và 481 token trích dẫn (đếm bằng `rg`, kể cả lặp; 03 chiếm 314 token số). Tôi
  không khử trùng đầy đủ, nên con số "khoảng 300 mục riêng biệt" là ước lượng.
- **Đã kiểm: 131 dòng, 418 mục** (tổng cột n, đếm bằng `awk` trên chính tệp này). Nguồn: PDF của 70 trên 72 arXiv id có PDF
  (chưa tra: 2210.10760, 2310.01798) cộng 7 PDF ngoài arXiv; 7 trang web gốc; `AGENTS.md`, 6 ADR, `design.md`. Tôi không tách số
  mục theo từng loại nguồn.
- **Kết quả theo dòng** (131, mỗi dòng đếm theo kết quả nặng nhất): CONFIRMED 88, WRONG 9 (#6, #10, #22, #47, #89, #94, #105,
  #106, #107), UNSUPPORTED 1 (#33), OVERSTATED 17 (gồm nhãn tin cậy quá cao và mâu thuẫn với ADR), UNVERIFIED 16. Mục 2 và 3 kiểm
  thêm 11 tham chiếu ADR/`AGENTS.md`, 11 mâu thuẫn và 1 điểm nhất quán (lịch đọc số), không tính vào 131 dòng.
- **Không kiểm được** (ghi UNVERIFIED, không đoán): văn bản luật Việt Nam (Luật 91/2025, 134/2025, Nghị định 142, 356, Quyết định
  33), EU (AI Act, Omnibus, PLD, GDPR, DSA), Mỹ (FTC, UETA, Colorado, EO, DGCL); điều khoản vendor (Anthropic, OpenAI); SEC
  (Knight); Vaccaro 2024; METR TH1.1, METR RCT, METR reward hacking blog (C56, C62); Epoch, a16z; Project Vend 2; blog multi-agent
  của Anthropic (90,2%, 4×, 15×); Anthropic 2026-10-27 (86%) và 93% phê duyệt; CBA (ia.acs.org.au); Bowyer 2503.01747 (không có PDF);
  bản v2 của MAST.
- **Có PDF nhưng chưa tra:** 2210.10760 (Gao), 2310.01798 (Huang), CRDT "5–10% xung đột ngữ nghĩa" (02:136, 03:311;
  2510.18893, 2608.23740); "1,8–2,3 lần" (C27) và "0,90" (C11) grep không ra, cần đọc bảng.
- **Phạm vi phản biện.** Bản này kiểm con số và trích dẫn, không kiểm lập luận thiết kế; các mục "Đề xuất của hội đồng (giả thuyết)"
  không có nguồn để kiểm và được để nguyên.
