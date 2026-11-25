# 02. Thiết kế hệ thống: công ty vận hành bằng agent (v2)

> Bản nháp của hội đồng, phần thiết kế hệ thống, bản 2 sau sáu phản biện. Ngày 2026-11-01. Bản 1: [`02-design-v1.md`](02-design-v1.md);
> nhật ký trả lời từng phát hiện: [`02-response.md`](02-response.md).
>
> **Cách trích.** [E §x], [D §x], [R §x], [P §x] = bốn báo cáo nghiên cứu (`research-ai-autonomous-system-{evidence,design,risk,pilot}.md`);
> [C1]–[C6] = `challenge-0{1..6}-*.md` của hội đồng. Mã arXiv trỏ tới PDF trong [`papers/`](../papers/INDEX.md) trừ khi ghi khác.
> Nhãn: **HIGH** (đo trực tiếp, đã đối chiếu), **MEDIUM** (suy ra hoặc nguồn một phía), **LOW** (phỏng đoán). **[HĐ-GT]** = *Đề xuất
> của hội đồng (giả thuyết)*. Không câu nào trong bản này nói một thành phần là "mới" hay "duy nhất"; nguồn gốc của từng phần ở mục 7.
> Nội dung pháp lý là nghiên cứu phục vụ thiết kế, không phải tư vấn pháp lý.

## 0. Đầu vào cố định, phán quyết, lập luận nền

**Quyết định của owner (đầu vào):** toàn hệ thống chạy cùng lúc bên trong một ranh giới; không thí nghiệm; không bí mật; không giới
hạn sáng kiến của agent (không mức tự chủ, chứng chỉ, trần chi phí, giới hạn phạm vi, cổng duyệt, dừng tự động theo hiệu năng).
**Điểm cố định:** (F1) nút dừng tay của owner; (F2) credential không vào ngữ cảnh agent; (F3) tuân thủ pháp luật.

**Phán quyết của chủ tọa** (mặc định, chờ owner xác nhận):
- **R1.** Các rail xác định của `AGENTS.md:47-51` (xác thực, pause và stop, merge; credential nhập vào trường chỉ ghi) giữ nguyên.
  Merge ở lại với owner vì bỏ nó thì agent đổi được chính mã của nút dừng (ADR-0012: hệ tự cải thiện qua chain của nó). "Không cổng
  duyệt" nghĩa là không cổng nào lên sáng kiến của agent bên trong nền tảng ngoài các rail đó. Gọi chung là **F4**.
- **R2.** "Mở" là mở với owner và người của công ty. Phương pháp được công bố ra ngoài sau khi một người có tên kiểm pháp lý.
  **Tập đánh giá niêm** (ngưỡng phát hiện, vị trí canary, tập neo giữ riêng, nhãn lỗi gieo) niêm với agent bị kiểm và với công chúng:
  ngoại lệ thứ hai bên cạnh credential.
- **R3.** Lặp cùng một tác vụ để đo độ nhất quán là quan sát (được). Nhánh ngẫu nhiên, holdout, A/B trên khách là thí nghiệm (không).
- **R4.** Luật thắng. Những gì luật buộc quay lại xuất hiện như hệ quả của F3 (mục 2.7).
- **R5.** Mỗi hiệu ứng được phân loại đảo ngược được hay không; với lớp không đảo ngược, điều kiện "tổn thất có cận" của `AGENTS.md:21`
  **không thoả** (mục 2.8). Hoãn gửi có huỷ là một lựa chọn, mặc định tắt.
- **R6, R7, R8, R10.** Không claim tính mới; Remit là hiện thực tham chiếu, không là bằng chứng (mục 3); sửa số theo [C1]; ranh
  giới là đơn vị doanh nghiệp tự chọn, đơn vị bên ngoài là nhóm so sánh quan sát (mục 2.6).

**Hệ quả phải nói thẳng.** Không giới hạn sáng kiến thì tổn thất của một lỗi chỉ phụ thuộc **thời gian phát hiện**, **thời gian dừng**
và **độ đầy đủ của phục hồi**. Với hiệu ứng đảo ngược được, ba đại lượng đó cho một cận đo được. Với hiệu ứng không đảo ngược (email
đã gửi, tiền đã chuyển, dữ liệu đã lộ, cam kết đã ràng buộc), phục hồi chỉ giới hạn **lần sau**, không trả lại **lần này** [C3 §4].
Knight Capital lỗ hơn 460 triệu USD trong khoảng 45 phút; SEC liệt kê việc thiếu giới hạn rủi ro tự chặn [E §5] (theo SEC qua báo cáo;
[C1] chưa tái kiểm trang SEC).

**Năng lực hiện có** giới hạn điều thiết kế hứa được: khoảng 30% tác vụ của TheAgentCompany (2412.14161), 2,5% dự án freelance thật
ở lần đo đầu (2510.26787); trong CEO-Bench mọi model thua baseline dựa trên quy tắc (2606.18543); độ tin cậy tăng 0,03/năm (GAIA) và
0,09/năm (τ-bench) trong khi độ chính xác tăng 0,23 và 0,22/năm (2602.16666) [P §2, §3] (HIGH).

**Đơn vị thiết kế** là một **ranh giới** do doanh nghiệp chọn (một tenant, một dòng sản phẩm, một đơn vị), chỉ gồm các hệ thống mà
gate phủ được [C2 pre-mortem 5]; mỗi ranh giới có một người chịu trách nhiệm có tên. Bên ngoài ranh giới không gì thay đổi.

## 1. Invariant kiểm được

| ID | Invariant | Phép thử bác bỏ | Nguồn, tin cậy |
|---|---|---|---|
| I1 | Mọi tác dụng ra ngoài của agent đi qua **một đường ra của nền tảng**; gate từ chối chỉ với lý do trong tập đóng `{STOPPED, UNLAWFUL, RAIL, PLATFORM_RECORD}` | Diễn tập đo **lưu lượng mạng của host**: một byte ra không có bản ghi gate là vi phạm; một từ chối ngoài tập đóng là giới hạn ẩn | Reference monitor, complete mediation [C6 §6]; hiện thực tham chiếu **chưa** đạt (mục 3) |
| I2 | Agent không bao giờ giữ credential | Canary credential gieo trong kho **và** quét mọi credential trên host (không chỉ giá trị trong kho); xuất hiện trong context, env, file đọc được, transcript là vi phạm | PocketOS [D §6]; ADR-0018:112-113 (khoá GitLab, SSH đọc được từ phiên) |
| I3 | Mọi hành động của agent hiện ở màn hình của agent đó cho owner trong thời hạn công bố; dữ liệu cá nhân trong đó hiện theo phân quyền mục đích (I17) | Hành động có trong ledger mà không có trên giao diện là vi phạm | `AGENTS.md:43-44`; BVDLCN Đ.30(3) [C4 §3.4] |
| I4 | Log, trace, ledger, bản sao lưu do nền tảng ghi, chỉ thêm, **chống giả mạo** (chuỗi băm), nằm nơi phiên không ghi được; mọi thao tác nhắm vào chúng nhận `PLATFORM_RECORD` | Thử ghi, xoá, ghi đè từ mọi công cụ; kiểm chuỗi băm hằng ngày | PocketOS [D §6]; Apollo 2412.04984 [C3 D7] |
| I5 | Nút dừng có hiệu lực trong thời hạn công bố theo ranh giới, cell, tenant, agent, nhà cung cấp model; dừng cả cây tiến trình | Diễn tập: dừng giữa lời gọi; đo ở mạng host tới gói tin cuối; đếm tiến trình con còn sống | Palisade 2509.14260; ADR-0021:63 (`setsid` sống qua deadline) [C3 §3.1] |
| I6 | Verdict có ít nhất một `evidence_ref` do nền tảng cấp | Verdict chỉ có văn bản bị schema từ chối | Loop-Back 2609.14767, Song 2603.16244 [D §3] |
| I7 | Finding chống một đề xuất mang `reason_code` trong tập đóng | Finding không mã là lỗi schema; tỷ lệ `OTHER` được báo cáo | MSR 2605.22534: 33,1% của 353 PR bị từ chối (mẫu 717 PR đọc tay) không có lý do quan sát được |
| I8 | Không vòng thương lượng: tác giả trả lời một finding một lần | Câu trả lời thứ hai bị schema từ chối | Song: nhiều vòng thêm 62% báo động nhầm (8,47 so với 5,23); 2608.18167 |
| I9 | Người kiểm và tác giả là hai pipeline; độ độc lập là số đo | Panel không có ρ và n_eff trên tập neo được tính là một phiếu | Hossain 2609.22512, Kohli 2605.29800 [E §2]; PoLL 2404.18796 [C6] |
| I10 | Số khách dựa vào do mã có phiên bản tính; mã **và cấu hình tham số** của hàm tính chỉ đổi qua merge (F4) | Truy mỗi con số gửi khách về một hàm và một băm cấu hình; không truy được là vi phạm | FinSheet-Bench 2603.07316 [P §1.5]; SOX §404 cho issuer niêm yết [C4 §3.5] |
| I11 | Mục tiêu do agent tự đặt không là đầu vào của hàm thưởng hay xếp hạng nào | Kiểu của báo cáo kết quả không có trường mục tiêu tự đặt | Google OKR guidance [D §9]; Apollo [R §3.1] |
| I12 | Mọi thay đổi prompt, playbook, harness, bộ nhớ chung, model, judge, checker là một **release** có băm; trace ghi băm đó; release ghi "đổi mức rủi ro pháp lý: có/không" do người có tên ký | Trace có băm ngoài sổ release là vi phạm | Hậu kiểm Anthropic [P §4]; Luật AI Đ.10(2) [C4 §3.1] |
| I13 | Ngữ cảnh chung đổi qua ghi có kiểu, theo mục, có phiên bản | API không có thao tác nhận cả tài liệu | ACE 2510.04618 [P §4] |
| I14 | Mọi tín hiệu phát hiện có người nhận, mức độ, trạng thái xử lý | Tín hiệu không người nhận là vi phạm | Knight: 97 email không được thiết kế như cảnh báo [E §5] |
| I15 | Mọi lệnh đổi trạng thái mang khoá idempotency | Phát lại ledger hai lần; so trạng thái | `AGENTS.md` (Idempotent by default) |
| I16 | Mã của gate, stop, ledger chỉ đổi qua merge; binary đang chạy dựng từ commit đã merge | So băm binary đang chạy với bản dựng lại | R1; cách so băm là [HĐ-GT] |
| I17 | Agent đọc dữ liệu cá nhân chỉ khi cặp (role, mục đích) có trong **sổ mục đích** do người có tên ký; mục đích mới là rail | `Resolve` trả `UNLAWFUL` cho mọi cặp ngoài sổ | BVDLCN Đ.3(2), 9(4)(a), 30(3); GDPR Đ.28(3)(a) [C4 §3.2] HIGH chữ |
| I18 | Mọi tin nhắn ra ngoài của agent tới người mang công bố "đây là AI" do gate gắn; nội dung sinh ra mang dấu máy đọc | Quét mẫu đầu ra ở gate | AI Act Đ.50(1),(2); Luật AI Đ.11 [C4 §4] HIGH chữ |
| I19 | Mỗi tham số của một hành động (người nhận, số tiền, URL) mang nguồn gốc; tham số rút từ nội dung ngoài hiện trong bản ghi và trên màn hình | Bản ghi hành động thiếu nguồn gốc tham số là vi phạm | MCPTox 2508.14925: đổi tham số là dạng hiệu quả nhất [C3 D3] |

## 2. Kiến trúc

### 2.1 Bốn mặt phẳng

```
 Owner ──(lời nói thường, đã xác thực)──> Assistant ──> Chair của cell
   ▲                                                     │ hợp đồng objective
   │ màn hình từng agent, hàng đợi ngoại lệ              ▼
 ┌─┴──────────── CONTROL PLANE (mã xác định) ──────────────────────────────┐
 │ đường ra duy nhất + gate · stop · ledger chống giả mạo · merge · F3      │
 └─▲───────────────▲────────────────────────▲─────────────────────▲────────┘
 AGENT PLANE     CONTEXT PLANE            VERIFICATION PLANE      OPERATING PLANE
 planner, đọc    mục có kiểu, phiên       checker xác định,       trace, sổ release,
 cách ly         bản, claim, nguồn gốc    auditor, tập niêm       ghim model, phục hồi
```

Control plane không đánh giá chất lượng công việc. Nó gắn credential, ghi, dừng khi owner bảo, ép các luật về luồng dữ liệu, và giữ
các rail. Tính hợp pháp của **nội dung** (một phát ngôn có phỉ báng không) không quyết được bằng hàm thuần; đó là rủi ro còn lại [C3 §3.4].

### 2.2 Vai trò

| Vai trò | Làm gì | Cơ sở |
|---|---|---|
| **Owner** / người chịu trách nhiệm ranh giới | Nói objective; kéo rail F4; ký hồ sơ pháp lý và sổ mục đích; đọc ngoại lệ; dừng tay | Người + AI kém bên tốt nhất đứng riêng (g = −0,23), hơn cả hai khi người giỏi hơn AI (g = +0,46) (Vaccaro, [R §2.1]; bài không có trong kho, MEDIUM) |
| **Assistant** | Thực hiện lời owner; mọi rail nó kéo trỏ tới **mã tin nhắn đã xác thực** của owner, hiện nguyên văn cạnh hành động | `AGENTS.md:47-49`; chống A25 [C3 D10] |
| **Chair** | Giữ hợp đồng objective và kế hoạch; điều phối claim; nhận finding | Magentic-One task ledger [D §2] |
| **Role agent** | Đề xuất OKR của role, thực thi, sửa theo finding | MetaGPT 2308.00352 (miền code) |
| **Đọc cách ly** | Đọc nội dung ngoài, trả **giá trị có kiểu**, không có công cụ hành động | CaMeL 2503.18813 (mục 2.9) |
| **Auditor** | Kiểm độc lập kết quả bằng nguồn thô; không thuộc đội; không có công cụ ghi vào thứ nó kiểm | Goal judge không thuộc đội (`docs/design.md:433`) |
| **Analyst** | Đọc KPI, surrogate, so sánh với đơn vị ngoài ranh giới | Remit Outcomes (`docs/design.md`) |

Số role không bị giới hạn bằng mã. Kiến trúc ngang hàng có kiểm chéo đạt 0,477 so với 0,466 của agent đơn nhưng 23,9 so với 67,7 thành
công trên 1.000 token (2512.08296 Bảng 5); việc tuần tự giảm 39–70% và trên khoảng 45% của agent đơn thì thêm agent có lợi ích âm (cùng
bài, phần hồi quy và theo benchmark) (HIGH cho số). Đó là thông tin trong brief của Chair và tín hiệu W2 (mục 9), không là luật.

### 2.3 Giao thức objective

1. **Hợp đồng objective.** Chair viết từ lời owner: khách hàng và điều họ nhận (ADR-0016), kết quả mong muốn, north-star, guardrail,
   surrogate theo id, chân trời, phần `unknown` kèm lý do.
2. **Đề xuất của role.** OKR, kế hoạch, và cho mỗi key result một `verification_method` máy chạy được. Brief của Chair theo nguyên tắc
   *contract-first* của Tomašev et al.: việc chưa kiểm được thì phân rã tiếp tới khi kiểm được (2602.11865, HIGH cho nội dung bài).
   Đây là phán đoán của agent trong brief, không là cổng: key result vẫn chạy được khi `unverifiable`, và tỷ lệ đó là tín hiệu W3.
3. **Kiểm chứng không chặn.** Checker xác định chạy ngay khi đề xuất được ghi: phủ yêu cầu của hợp đồng cha, metric tồn tại và chưa hết
   hạn, claim chồng lấn, guardrail. Chair ghi finding có `reason_code` và `evidence_refs`. Thực thi không chờ; tác giả trả lời một lần,
   chưa giải quyết thì vào hàng đợi ngoại lệ (I8). Ngoại lệ là rail F4: thay đổi chạm số khách dựa vào chỉ vào production qua merge (I10).
4. **Thực thi.** Key result sinh task; task khai claim. Rút claim và trễ cam kết vào hồ sơ role (dữ liệu để đọc) [D §2].

`reason_code`: `UNCOVERED_REQUIREMENT`, `UNVERIFIABLE_KR`, `UNKNOWN_METRIC`, `OVERLAP`, `GUARDRAIL_BREACH`, `EVIDENCE_CONTRADICTS`,
`PROCESS_CONSTRAINT`, `DUPLICATE`, `OTHER(text)`. `PROCESS_CONSTRAINT` tách riêng vì 31,2% của 353 PR bị từ chối trong mẫu 717 PR đọc
tay của MSR là do quy trình (2605.22534).

Giới hạn của bằng chứng: Loop-Back (43 cặp sản phẩm, 86 lần chạy, một loại việc) cho thấy quyền gửi lại để sửa có hại khi người duyệt
chỉ góp ý và có ích "when the Manager can verify the work" (2609.14767; HIGH cho số, MEDIUM cho khái quát). Kiến trúc tập trung giữ
khuếch đại lỗi ở 4,4 lần nhờ chặn lỗi trước khi gộp, so với 17,2 lần khi độc lập (2512.08296). Thiết kế không chặn bỏ đúng phần lợi ích
đó [C2 P1]; giá trị còn lại của kiểm chứng là rút ngắn thời gian tới lúc sửa, và nó chỉ đủ với hiệu ứng đảo ngược được (mục 2.8).

### 2.4 Lớp ngữ cảnh chung

| Thành phần | Đặc tả | Cơ sở |
|---|---|---|
| **Mục có kiểu** | JSON schema, `version`, `provenance` (ai ghi, từ đầu vào nào, tin cậy hay không), `DataClasses`, tóm tắt ở đầu | Harness có schema của Anthropic [D §5]; LangGraph state [C6] |
| **Ghi** | `Write(section, base_version, patch)`; `base_version` cũ bị từ chối kèm diff (tính đúng, không phải quyền); mục chứa dữ liệu cá nhân ghi và đọc theo I17 | ACE [P §4]; optimistic concurrency kiểu ETag [C6 E2] |
| **Claim** | Công bố ai đang làm gì; chồng lấn được báo cho hai bên và Chair, không bị từ chối | Xung đột ở 48/115 cặp PR khác agent so với 119/601 cặp cùng agent (2607.04697) |
| **Phép chiếu** | Mỗi agent nhận phạm vi của nó cộng tóm tắt phạm vi người khác | −39% khi nhiều lượt (2505.06120) |
| **Nguồn gốc** | Nhãn `untrusted` gắn ở **mục và tham số** (I19), không lan theo phiên: bản 1 lan taint theo phiên làm mọi phiên nhiễm trong vài giờ và tín hiệu chết [C3 A10]; gắn nhãn đứng riêng không đủ chặn lây nhiễm (2410.07283) | [C3 D3] |
| **Bộ nhớ dài hạn** | Một mục có kiểu; đổi bộ nhớ chung là release | 2509.26354: tỷ lệ từ chối 99,4% → 54,4% |

Bằng chứng chống: OCC và khoá không hợp với suy luận dài; CRDT vẫn để xung đột ngữ nghĩa (2510.18893, 2608.23740; [C1] chưa tra con số
5–10%) [D §5]. Bộ nguồn không có số đo xung đột trên tài liệu kế hoạch (LOW).

### 2.5 Lớp kiểm chứng

Thứ tự ưu tiên khi bất đồng: **checker xác định → auditor độc lập → neo hiệu chỉnh**. Kiểm chứng báo cáo và sinh việc sửa; nó không chặn
sáng kiến, trừ khi đi qua rail F4 hoặc F3 (merge cho số khách dựa vào; quyết định luật bắt có người, mục 2.7).

- **Tầng 0, checker xác định.** Test, schema, bất biến số học, đối soát nguồn thô, chạy song song mã cũ và mới (GitHub Scientist [D §7]).
  Đây là thực hành chuẩn (PAL, Program of Thoughts, data lineage, đối soát trong kiểm soát nội bộ [C6 E4]), không là đóng góp. Checker
  cho đầu ra của một role do pipeline khác viết; checker chạm số tài chính đổi qua merge (tách người làm và người kiểm cho ICFR [C5 §2]).
- **Tầng 1, auditor độc lập.** Panel trộn judge frontier với open-weight: ba judge frontier khác nhà cung cấp có ρ = 0,56 và cùng sai gấp
  7,7 lần mức độc lập (2609.22512); 9 judge chỉ bằng khoảng 2 phiếu độc lập (2605.29800) (HIGH). Gộp có tính phụ thuộc (2604.07650:
  +3,5 điểm so với đa số). Hoán đổi thứ tự A/B (Zheng et al. [P §5.1]); đo κ thay tỷ lệ khớp thô, vốn thổi phồng 33–41 điểm (2606.19544).
  Auditor đọc artifact như dữ liệu `untrusted` (B7) và có thể bị lừa bằng chuỗi hời hợt, tới 80% báo nhầm (2507.08794) [C3 A27].
- **Tầng 2, neo hiệu chỉnh.** Tập neo do người gán nhãn, giữ riêng trước khi giám sát bắt đầu, niêm (R2). Trong 2606.15474 (một tác giả),
  **một** lần đổi phiên bản judge âm thầm được tập neo phát hiện ở 60/60 lần chạy, còn z-test cuốn chiếu báo nhầm trên 75% chuỗi không đổi;
  đổi judge làm đổi 59% tập top-30% (2610.32986).
- **Tỷ lệ bắt lỗi** của checker và auditor đo bằng lỗi gieo từ lịch sử, **trước ngày 0** trên tác vụ ghi lại và liên tục sau đó (R3: quan
  sát), vì generator càng mạnh càng khó kiểm (2509.17995) [C2 pre-mortem 6].
- **Quy trình cùng kết quả**: 80,1% số lần đạt kết quả của CXBench vi phạm quy trình (55,5% của mọi 840 lần chạy) (2610.25954).
  **Độ nhất quán**: pass^k và lần tệ nhất trên tối thiểu K = 3 lần lặp [P §5.2] (`04-launch.md` dùng K = 5, nằm trong mức đó).

**Lặp trên dữ liệu khách là một mục đích xử lý.** Dùng dữ liệu khách để tạo tác vụ ghi lại, bộ hồi quy hay tập neo cần cơ sở pháp lý
cho mục đích đó và nằm trong sổ mục đích (I17) [C4 §6].

### 2.6 Thưởng, KPI, và đo nhân quả không thí nghiệm

| Loại | Dùng để | Không dùng để |
|---|---|---|
| **North-star** (doanh thu, giữ chân) | Đọc sức khoẻ ranh giới theo quý | Thưởng trực tiếp |
| **Surrogate** (gắn một loại thay đổi, có chủ, có hạn) | Đọc tác động một loại thay đổi | Loại khác; sau hạn |
| **Guardrail** (độ chính xác số liệu, khiếu nại, huỷ) | Báo động; ngoại lệ | Tối ưu |
| **Tự đặt** (mục tiêu OKR của role) | Đo hiệu chỉnh | Thưởng, xếp hạng (I11) |

Surrogate có chủ, loại thay đổi, bằng chứng hiệu lực, ngày hết hạn. Sigerson et al. nêu surrogate chỉ tốt trong cùng loại thay đổi, và lấy
ví dụ giả định về khai thác (gửi thông báo mỗi 7 ngày để kéo một tương quan ngắn hạn) (2608.08043). Trên 200 A/B test (1.098 arm) của
Netflix, surrogate 14 ngày có recall 65% với quyết định phát hành thật (2311.11922; bài tự mâu thuẫn giữa precision và recall). KPI sai
dẫn hành động sai, vì agent tối ưu theo nó (Gao 2210.10760) [C2 §5].

**Nhân quả không thí nghiệm, nói đúng mức.** Thí nghiệm dài hạn là cách tốt nhất để học và kiểm surrogate (2608.08043), và surrogate index
của Athey cần mẫu có gán ngẫu nhiên (NBER w26463) [C2 P2]; R3 loại cả hai. Còn lại các phương pháp quan sát, yếu hơn thí nghiệm nhưng khác
0 **nếu có đơn vị so sánh**: chênh lệch trong chênh lệch (2607.01904 dùng staggered DiD trên dữ liệu quan sát), synthetic control (Abadie et
al., JASA 2010), Bayesian structural time series (Brodersen et al., 2015) [C6 §4, MEDIUM]. Bật toàn bộ cùng lúc bên trong ranh giới xoá so
sánh bên trong; **đơn vị ngoài ranh giới** (R10) là nhóm so sánh. Thiết kế đòi: chiến lược nhận dạng công bố trước ngày 0 (đơn vị so sánh,
chỉ số, cửa sổ, ngưỡng); không có đơn vị ngoài thì mọi đánh giá north-star chỉ là tương quan theo thời gian (LOW cho nhân quả). Hiệu chỉnh
surrogate bằng kết quả quan sát nằm ngoài điều kiện nhận dạng của Athey và được ghi như vậy.

**Thử thước đo** trên đường ống đo bằng dữ liệu phát lại có lỗi chủ ý, không trên khách, để biết metric còn phản ứng (Bing: lỗi làm doanh
thu mỗi người dùng tăng hơn 30%, Kohavi 2012). Đây là kiểm tra dụng cụ theo R3 [HĐ-GT cho cách làm].

**Detector không vào thưởng** (Baker 2503.11926: monitor vào reward thì bị né). Claim tự báo về đóng góp được kiểm mẫu (2610.33728, MEDIUM).

### 2.7 Điểm cố định, rail, và hệ quả bắt buộc của luật (F3)

| Mục | Hiện thực | Lý do từ chối ở gate |
|---|---|---|
| **F1 Dừng tay** | `StopState` theo ranh giới, cell, tenant, agent, nhà cung cấp; dừng cả cây tiến trình; đo ở mạng host (I5) | `STOPPED` |
| **F2 Credential kín** | Executor (backend, proxy chèn credential, jail) gắn secret; agent giữ tên connection. `Redact` chống rò **vô ý**; nó không chống chương trình cố ý mã hoá [C3 §3.2] | — |
| **F4 Rail `AGENTS.md`** | Xác thực; pause và stop; merge do owner (hoặc Assistant theo tin nhắn đã xác thực) | `RAIL` |
| **Bản ghi nền tảng** | Log, ledger, sao lưu không là đích của công cụ nào (I4) | `PLATFORM_RECORD` |

**Hệ quả của F3** (theo [C4], HIGH cho chữ văn bản đã đọc, MEDIUM cho cách áp dụng; mỗi mục là `UNLAWFUL` ở gate hoặc một rail người ký):

| Luật đòi | Thiết kế | Nguồn |
|---|---|---|
| Kiểm soát của người ở chỗ luật bắt | Chức năng rơi vào rủi ro cao hoặc quyết định "solely" tự động có hiệu lực pháp lý với thể nhân, quyết định về việc làm của một người cụ thể: cần quyết định của người có tên; gate trả `UNLAWFUL` tới khi có | AI Act Đ.14(4)(d), Annex III 4(b); GDPR Đ.22; Luật AI Đ.4(2), 14(1)(d) [C4 §3.2] |
| Giới hạn mục đích và quyền truy cập dữ liệu cá nhân | Sổ mục đích, I17; mục đích mới là rail người ký | BVDLCN Đ.3(2), 9(4)(a), 30(3); GDPR Đ.28(3)(a), 28(10) |
| Công bố AI với người ngoài | I18, gắn bằng mã ở gate | AI Act Đ.50(1),(2),(5); Luật AI Đ.11 |
| Hồ sơ ký bởi người có tên | Agent soạn; người đại diện theo pháp luật ký và nộp: phân loại và thông báo **trước khi dùng**, hồ sơ chuyển dữ liệu và đánh giá tác động xử lý trong 60 ngày; chỉ định nhân sự bảo vệ dữ liệu (BVDLCN Đ.33(2)); hệ thống bật sau 01/03/2026 không có thời gian chuyển tiếp; chức năng rủi ro cao được test theo chỉ số định trước trước khi dùng (AI Act Đ.9(8), Luật AI Đ.13(3)), trên tác vụ ghi lại (R3) | Luật AI Đ.10, 35(1); NĐ 142 Đ.14; BVDLCN Đ.20, 21; LDN 2020 Đ.12(1) [C4 K9, K12] |
| Thủ tục lao động | Thay người không xảy ra ở ngày 0: trao đổi với tổ chức đại diện, báo trước 30 ngày (Việt Nam), tham vấn (EU), WARN 60 ngày (Mỹ), đồng quyết hệ thống giám sát (Đức) | BLLĐ Đ.42; 98/59/EC; 29 U.S.C. §2101; BetrVG §87(1) Nr. 6 |
| Kiểm soát thay đổi SOX (issuer niêm yết) | Mọi release, kể cả prompt và cấu hình, làm đổi một con số tài chính đi qua merge với bằng chứng duyệt | SOX §404 [C4 §3.5] |
| Báo cáo sự cố nghiêm trọng | Sự cố chạm ngưỡng luật mở một mục có hạn chót (72 giờ, 05 ngày làm việc) do người có tên ký; báo cơ quan trước khi công bố postmortem | BVDLCN Đ.23; NĐ 142 Đ.19; GDPR Đ.33 |

Ở các chỗ này "không cổng duyệt" nhường chỗ (R4). Câu hỏi luật sư số 1 của [C4 §5] (nút dừng, log và quyền đảo ngược sau có đủ cho Luật
AI Đ.4(2) không) là điều kiện bật các chức năng tương ứng, không phải câu hỏi mở sau khi bật.

### 2.8 Lớp hiệu ứng theo khả năng đảo ngược (R5)

| Lớp | Ví dụ | Phục hồi đã xây | Cận tổn thất |
|---|---|---|---|
| **H0 đảo ngược trong nền tảng** | Mã, cấu hình, mục ngữ cảnh, dữ liệu nội bộ có sao lưu | Revert theo băm; phát lại ledger; khôi phục; **bao đảo ngược mặc định** (xoá mềm, snapshot trước khi ghi đè, bản ghi bù trừ) [C3 D5] | Có: thời gian phát hiện + thời gian dừng + cửa sổ khôi phục, mỗi số đo bằng diễn tập |
| **H1 bù trừ được** | Con số khách thấy (sửa kèm thông báo), tín dụng chưa chi trả, giá trong catalog | Bản ghi bù trừ, thông báo người bị ảnh hưởng | Có, cộng chi phí bù trừ; tổn thất niềm tin không đo |
| **H2 không đảo ngược** | Email, tin nhắn, phát ngôn công khai, tiền chuyển ra, dữ liệu gửi bên thứ ba hoặc nhà cung cấp, xoá ở hệ thống bên thứ ba, cam kết ràng buộc (UETA §14; Luật GDĐT Đ.14(2)(a)) | Không có cái gì trả lại lần này | **Không.** Điều kiện của `AGENTS.md:21` không thoả; rủi ro được nhận không có cận theo quyết định của owner |

Với H2, các biện pháp không là giới hạn: tách luồng điều khiển khỏi dữ liệu (2.9), nguồn gốc tham số (I19), tripwire theo **tổng trong cửa
sổ** để chống chia nhỏ [C3 D6], và **hoãn gửi có huỷ** [C3 D4]: nền tảng giữ hành động H2 một cửa sổ ngắn, F1 trong cửa sổ đó rút được;
không ai phải duyệt. Theo R5 nó là lựa chọn, **mặc định tắt**; bật nó là quyết định của owner. Doanh nghiệp áp dụng ký một **tuyên bố rủi
ro còn lại** liệt kê các lớp H2 không có cận và người cấp điều hành chấp nhận từng lớp [C5 §5]: không thêm giới hạn, chỉ ghi ai chấp nhận.

### 2.9 Credential, đường ra, prompt injection

Tấn công thích ứng vượt 12 phòng thủ tĩnh với tỷ lệ trên 90% cho đa số (2510.09023); nội dung web khiến hệ multi-agent chạy mã tuỳ ý 58–90%
(2503.12188); orchestrator không chạm nội dung ngoài vẫn bị chiếm qua sub-agent [C3 A7] (HIGH). Các thiết kế có số đo trên AgentDojo dựa vào
kiểm soát luồng thông tin: CaMeL (2503.18813) và FIDES (Costa et al., arXiv 2505.23643, "complete a broad range of tasks with security
guarantees" theo abstract; không có trong kho, MEDIUM). Phần **chính sách** của chúng (và Rule of Two của Meta [D §6]) là giới hạn, nên bị
loại theo quyết định của owner. Phần **tách luồng** thì không. Phép thử dùng: một biện pháp là giới hạn nếu nó khiến hành động agent đã chọn
không xảy ra hoặc phải chờ chủ thể khác [C3 §1].

| # | Thay đổi | Giới hạn? | Vì sao | Bằng chứng |
|---|---|---|---|---|
| D1 | **Tách luồng điều khiển khỏi dữ liệu**: planner chỉ thấy đầu vào tin cậy (lời owner, brief, hợp đồng, mục có nguồn tin cậy); nội dung ngoài qua đọc cách ly, chỉ thành giá trị có kiểu, không thành lệnh | Không | Mọi hành động vẫn chọn được; chỉ đổi ai được ra lệnh | CaMeL không cài chính sách vẫn cải thiện an ninh mọi model trên 949 tấn công AgentDojo (Hình 9). Giới hạn: role mà việc chính là làm theo lời khách vẫn biến dữ liệu thành lệnh [C3 D1] |
| D2 | **Một đường ra của nền tảng** cho phiên và chương trình; F1, F3 ép ở đó | Không | Không đích nào bị chặn; chỉ đổi kênh | Điều kiện để I1, I5 đúng (mục 3) |
| D3 | Nguồn gốc theo tham số (I19) | Không | Ghi và hiển thị | 2508.14925 |
| D4 | Hoãn gửi có huỷ cho H2 (mặc định tắt) | Không nếu không ai phải duyệt | Hành động chắc chắn xảy ra trừ khi bị dừng tay | Tổn thất theo thời gian dừng (Knight) |
| D5 | Bao đảo ngược mặc định | Không | Hành động vẫn xảy ra | PocketOS, Replit (nguồn mâu thuẫn về khôi phục [C1 #33]) |
| D6 | Tripwire theo tổng trong cửa sổ | Không: báo | Chống chia nhỏ | [C3 A20] |
| D7 | Log chống giả mạo (I4) | Không | Chỉ ghi | Apollo 2412.04984 |
| D8 | Canary token (credential giả, bản ghi khách giả), vị trí niêm (R2) | Không: báo | Vô hiệu nếu vị trí công khai | 2510.09023 (kẻ thích ứng dùng phản hồi của detector) |
| D10 | Rail do Assistant kéo trỏ tới tin nhắn đã xác thực | Không: là xác thực | `AGENTS.md:47-49` | [C3 A25] |
| D11 | Dừng đo ở mạng host | Không: phép đo | Log của gate chỉ chứng minh đường qua gate | [C3 §3.1] |
| D12 | Checker đọc mã và mô tả máy chủ MCP, và công cụ do agent tạo, ở lần đầu đăng ký; băm sau đó | Không: báo | Băm chỉ thấy thay đổi; MCPTox đầu độc từ đầu | 2508.14925 |

Egress không có allowlist đích; đích mới là tripwire. Nhà cung cấp model dự phòng chỉ được dùng khi có DPA (I17, F3) [C3 A22].

### 2.10 Lớp vận hành

| Thành phần | Đặc tả | Cơ sở |
|---|---|---|
| **Trace** | Bước, lời gọi công cụ, verdict: phiên, role, `config_hash`, mã model do API trả về, connection, nguồn gốc tham số, kết quả gate. OTel GenAI khi cần (quy ước đang ở trạng thái Development) | [P §4] MEDIUM |
| **Kiểm release** | Trước khi release có hiệu lực: eval trên tác vụ ghi lại, K ≥ 3, so phân phối (trung bình, pass^k, lần tệ nhất; Wilson khi mẫu nhỏ). Kết quả xấu là ngoại lệ và việc của tác giả; release chạm số tài chính hoặc mã rail đi qua merge | Miller 2411.00640; ba thay đổi harness Anthropic không được eval tái hiện [P §4] |
| **Theo dõi sau release** | Phân phối theo `config_hash` trên lưu lượng thật | Lỗi định tuyến: 16% yêu cầu Sonnet 4 ở giờ tệ nhất ngày 31/8, phát hiện qua phản hồi người dùng [P §4] |
| **Ghim model** | Snapshot có ngày; lịch ngừng (Anthropic báo trước tối thiểu 60 ngày, MEDIUM); chạy lại eval khi có thông báo | [P §4] |
| **Phục hồi** | Mục 2.8, H0; diễn tập có đo thời gian và biên bản | [C5 §5 #5] |
| **Quy tắc tiến độ** | Không tiến triển sau N vòng thì Chair được báo, owner thấy | Vending-Bench [E §4] |

### 2.11 Giao diện của owner

94% (83/88) lập trình viên **không có monitor** không phát hiện phá hoại; có monitor báo đúng, phá hoại vẫn thành công 9/16 (2606.05647)
(HIGH). Người trùng với khuynh hướng của agent 86% (Anthropic 2026-10-27, HIGH theo [R §2.1], [C1] chưa tái kiểm). 93% lời nhắc xin quyền của
Claude Code được chấp nhận (MEDIUM-HIGH); áp sang owner duyệt merge là suy rộng. Vì vậy:

1. **Màn hình từng agent** có mọi hành động (I3).
2. **Hàng đợi ngoại lệ** máy xếp theo rủi ro và lớp hiệu ứng, loại trùng. Chuyển cho người chỉ ca đã gắn cờ tăng TPR khoảng 15% ở FPR 0,01,
   và cùng bài kết luận "lack of adversarial robustness for LLMs and humans" (2508.19461). Mục tiêu đo được: với mọi lỗi gieo lớp H2, ngoại
   lệ tới hàng đợi trong ≤ T phút mà owner không mở màn hình agent; T công bố trước ngày 0 và niêm theo R2 khi là ngưỡng phát hiện.
3. **Bằng chứng do nền tảng dựng** (diff, kết quả checker, con số do mã tính, nguồn gốc tham số), tách khỏi lời agent (Vasconcelos
   2212.06823) [HĐ-GT cho cách tách].
4. **Lập luận mạnh nhất của mỗi lựa chọn** [R §2.3]; **kết quả của những gì đã merge**: tỷ lệ merge không bình luận (Outcomes đã đo) và
   lỗi tìm thấy sau merge (chưa có).

## 3. Hiện thực tham chiếu: yêu cầu ngày 0 chưa có (R7)

Remit là hiện thực tham chiếu, không là bằng chứng hiệu quả. Hôm nay nó **chưa** thoả các invariant sau; mỗi dòng là một yêu cầu phải
xây trước ngày 0, không phải một sự thật:

| Khoảng trống | Nguồn | Invariant | Yêu cầu |
|---|---|---|---|
| Phiên giữ mạng trực tiếp tới nhà cung cấp, ngoài gate | ADR-0018:71 | I1, I5, I17 | D2: một đường ra |
| Hồ sơ Seatbelt macOS cho phép mặc định | `internal/jail/sandbox_darwin.go:143` `(allow default)` | I1 | D2 |
| Tiến trình con `setsid` sống qua deadline (không có pid namespace trên macOS) | ADR-0021:63 | I5 | Dừng cả cây tiến trình, đo ở host |
| Khoá GitLab và SSH đọc được từ phiên | ADR-0018:112-113 | I2 | Connection kind GitLab và SSH sau gate |
| Phiên đọc môi trường của server và mọi tiến trình cùng user | ADR-0021:66 | I2 | User OS riêng cho phiên (ADR-0004) |
| Bỏ "Least grant" và "grant `write` chờ owner" của ADR-0018 (invariant 2) theo quyết định của owner; luật vẫn đòi phân quyền dữ liệu cá nhân | ADR-0018:40-43 | I17 | ADR thay thế ADR-0018 invariant 2; chờ owner |
| Một nhà cung cấp model (ADR-0006); một owner mỗi server (ADR-0022) | ADR-0006, ADR-0022 | — | Với doanh nghiệp: đường đa nhà cung cấp viết theo DORA, định danh nhiều người [C5 §5] |

## 4. Hệ thống tối thiểu đủ để phát hiện, dừng và phục hồi

"Tối thiểu" nghĩa là: thiếu nó thì lớp H0 và H1 mất cận ở ngày 0. **Tổn thất của lớp H2 vẫn không có cận** (mục 2.8).

| Thành phần | Hạng | Vì sao |
|---|---|---|
| D2 một đường ra; F1 theo mọi mức, dừng cả cây tiến trình, diễn tập đo ở mạng host (I1, I5, D11) | **Ngày 0** | Thời gian dừng là cận của tổn thất đang diễn ra |
| F2 cho **mọi** credential trên host; canary credential (I2, D8) | **Ngày 0** | Lộ secret không phục hồi được |
| Ledger chống giả mạo, sao lưu ngoài công cụ, bao đảo ngược, diễn tập khôi phục (I4, I15, D5, D7) | **Ngày 0** | "Sửa khi chạy" cần trạng thái để quay về |
| Trace có `config_hash`, mã model thật, nguồn gốc tham số (I3, I12, I19) | **Ngày 0** | Không có thì không chẩn đoán được |
| Hệ quả F3: sổ mục đích, công bố AI, hồ sơ đã ký và nộp trước khi dùng, quy trình báo sự cố (I17, I18, 2.7) | **Ngày 0** | Luật đòi trước khi dùng |
| F4 xác thực, merge; I10, I16 | **Ngày 0** | `AGENTS.md`, R1 |
| D1 tách luồng cho Chair và mọi role có công cụ H2 | **Ngày 0** | Biện pháp có số đo trước H2 mà không là giới hạn (CaMeL không chính sách, Hình 9) |
| Tripwire có người nhận, theo tổng trong cửa sổ; hàng đợi ngoại lệ (I14, D6) | **Ngày 0** | Tín hiệu không người nhận không tồn tại |
| Tỷ lệ bắt lỗi của checker và auditor và thời gian phát hiện theo lớp hiệu ứng, đo bằng lỗi gieo trên tác vụ ghi lại | **Ngày 0** | Không có thì tầng kiểm chứng là ẩn số [C2 P4] |
| Tập đánh giá niêm (R2); chiến lược nhận dạng công bố (2.6); tuyên bố rủi ro còn lại đã ký (2.8) | **Ngày 0** | Không có thì số đo lạc quan và nhân quả không có cơ sở |
| Thành phần panel, ρ và n_eff đầy đủ, tập `reason_code`, schema mục, phép chiếu, registry surrogate, độ rộng bộ eval | Tinh chỉnh | Thiếu thì yếu hơn; tầng 0 và các mục trên vẫn đứng |

## 5. Luồng chính và cách chúng hỏng

| Luồng | Trạng thái | Cách hỏng chính → sửa tại chỗ |
|---|---|---|
| **L1 Objective** | `DRAFT → CONTRACTED → ACTIVE → MEASURING → CLOSED`; mọi trạng thái → `STOPPED` | Hợp đồng mơ hồ (System Design Issues 41,8% trong MAST v3, Hình 4, 210 trace) → Chair viết lại mục hợp đồng, brief là release. `MEASURING` đóng khi **goal judge** xác nhận (ADR-0013:33-35) |
| **L2 Đề xuất và finding** | `SUBMITTED → CHECKED(T0) → FINDINGS → ANSWERED → RESOLVED \| EXCEPTION`; không có `ANSWERED → ANSWERED` | Chair cho qua máy móc (Vend 2: CEO agent duyệt gấp khoảng 8 lần số từ chối, MEDIUM) → lỗi gieo vào đề xuất, đổi brief; reviewer bịa lỗi (2407.00215) → I6 |
| **L3 Task** | `CLAIMED → WORKING → SUBMITTED → VERIFIED \| FINDINGS → MERGED(F4, nếu mã) → OBSERVING → DONE \| REVERTED`; chỉ checker hoặc auditor đặt `VERIFIED` | Số sai âm thầm → đối soát, chạy song song, revert theo băm; nợ tích tụ (2511.04427) → chain dọn nợ |
| **L4 Release** | `PROPOSED → CHECKED(K lần) → LIVE → OBSERVED`; `LIVE → REVERTED` bởi tác giả, Chair, owner | Eval không tái hiện suy giảm → theo dõi theo `config_hash`; đổi judge ngầm đổi thước đo → judge là release riêng |
| **L5 Dừng** | `STOP(owner) → đường ra đóng → cây tiến trình dừng → lưu → STOPPED → RESUME(owner)` | Đường ra ngoài gate, tiến trình con còn sống (mục 3) → D2, I5 đo ở host; lệnh đã gửi → `IN_FLIGHT` và khoá idempotency; mất control plane → mặc định đóng [HĐ-GT]. "Không có đường ra sau `STOPPED`" là **kết quả đo** của N diễn tập, không là sự thật |
| **L6 Hành động** | `REQUEST → Resolve → EXECUTE \| STOPPED \| UNLAWFUL \| RAIL \| PLATFORM_RECORD`; `EXECUTE → [HOLD nếu D4 bật] → executor → che → ghi → Tripwires` | Confused deputy: phiên bị injection dùng connection hợp lệ → D1 trước, I19 và D6 phát hiện, F1 và phục hồi; với H2 không có gì trả lại |

## 6. Giao diện để hiện thực

Pseudo-Go; tên là đề xuất, cấu trúc là đặc tả. Hàm `pure` không làm I/O.

```go
type Effect string     // read | write | deploy | merge | commit_to_customer | spend | send | delete
type EffectClass int   // H0 reversible | H1 compensable | H2 irreversible
type Refusal string    // STOPPED | UNLAWFUL | RAIL | PLATFORM_RECORD   (closed set, I1)

type Param struct{ Name, Value string; From []Source /* trusted | untrusted origin ids */ }
type Action struct {
    Session SessionID; Actor Principal; Role, Cell, Tenant, Connection, Provider string
    Effect Effect; Class EffectClass; Params []Param; DataClasses []DataClass; Purpose PurposeID
    Target Target; IdempotencyKey, ConfigHash string; OwnerMessage *MessageID // set when the Assistant pulls a rail
}
type Resolution struct{ Execute bool; Refusal Refusal; Reason string }

func Resolve(a Action, s StopState, law LegalPolicy, rails RailSet, records RecordSet) Resolution // pure
func Tripwires(a Action, h Window, cfg SealedThresholds) []Alert                                 // pure; never input to Resolve
func Disclose(msg Outbound) Outbound                                                             // pure; I18
func Redact(result []byte, known SecretSet) []byte                                               // pure; accidental leaks only

type Finding struct {
    Subject ProposalID; Version int; Code ReasonCode; Evidence []EvidenceRef // platform-issued
    RequiredChanges []Requirement; Reviewer Pipeline; Answer *Answer          // at most one
}
func ValidateFinding(f Finding, author Pipeline) error // pure; I6–I9

type Section struct{ ID string; Schema SchemaID; Version int; DataClasses []DataClass; Provenance []Source }
func ApplyWrite(s Section, base int, p Patch, w Principal, purposes PurposeRegister) (Section, *Conflict, error) // pure; I13, I17
func Overlaps(held []Claim, want Claim, now time.Time) []Claim                                                 // pure; reported only

type Purpose struct{ ID string; Roles []string; DataClasses []DataClass; LegalBasis string; SignedBy PersonID }
type Release struct{ ConfigHash string; Parts map[string]string; Check EvalResult; RiskClassChanged bool; SignedBy *PersonID }
type SealedSet struct{ Anchors, SeededLabels, CanaryPositions, Thresholds Ref } // unreadable by any agent under test (R2)
type OutcomeReport struct{ Role string; Verified []VerdictID; PassK map[TaskID]float64 } // no self-set target (I11)
```

**Thuộc tính cần test** (property test trên hàm thuần):
- `Resolve`: với mọi `Action`, `!Execute` ⇒ `Refusal` thuộc tập đóng, và mỗi lý do có điều kiện kích hoạt riêng: `STOPPED` ⇔ phạm vi trong
  `StopState`; `RAIL` ⇔ `Effect = merge` (hoặc rail khác) mà `Actor` không phải owner, hoặc Assistant không có `OwnerMessage` đã xác thực;
  `PLATFORM_RECORD` ⇔ `Target` thuộc `RecordSet`; `UNLAWFUL` ⇔ `LegalPolicy` trả vi phạm (I17, nhà cung cấp không có DPA, quyết định luật
  bắt có người). Ngoài bốn điều kiện đó, kết quả là `Execute`. Phép thử này chứng minh `Resolve` không chứa giới hạn ẩn; nó không chứng minh
  gì về các đường khác (scope của connection, rate limit của nhà cung cấp, sandbox), vốn cần diễn tập I1 [C6 #14].
- `Tripwires` không là đầu vào của `Resolve` (đồ thị phụ thuộc package).
- `Disclose`: mọi `Outbound` tới người có công bố AI ở đầu; `Redact`: secret đã biết nhúng trong kết quả không còn ở đầu ra, kể cả base64 và
  URL-encode (chống rò vô ý, không chống mã hoá cố ý).
- `ValidateFinding`, `ApplyWrite`: như I6–I9, I13, I17; phát lại ledger hai lần cho cùng trạng thái (I15).

**Test chạy trong production**, kết quả hiện cho owner: diễn tập đường ra và dừng đo ở mạng host (I1, I5), quét mọi credential (I2), diễn tập
khôi phục (I4), kiểm chuỗi băm (I4), so băm binary rail (I16), lỗi gieo vào checker và auditor, tính lại ρ và n_eff khi có release judge.

## 7. Bằng chứng và prior art

Không thành phần nào là phát minh [C6 §2]. Phần riêng là **tổ hợp** các lựa chọn đứng cùng nhau, chưa được chứng minh có ích.

| Phần tử | Prior art | Bằng chứng ủng hộ | Bằng chứng chống hoặc giới hạn | Tin cậy |
|---|---|---|---|---|
| Gate, đường ra duy nhất | Reference monitor (Anderson 1972), complete mediation (Saltzer, Schroeder 1975), agentgateway, Agent 365 [C6] | PocketOS, Replit [D §6] | Mọi khung đã công bố (IMDA 2026, Google, Shavit 2023, NIST NCCoE, OWASP) khuyên giới hạn quyền; gate chỉ từ chối vì dừng, luật, rail và bản ghi nền tảng là đi ngược các khung đó [C6 E3] | MEDIUM |
| Không giới hạn sáng kiến | Paperclip có trần chi phí và cổng duyệt [C6] | Không có số đo trực tiếp; lập trường nhận rủi ro | SEC (Knight); Vend 2 cần giá sàn, hạn mức chi (MEDIUM); Antigravity; big bang ERP Hershey 1999 (MEDIUM) [C6 E8] | LOW rằng tổn thất H2 có cận |
| Credential kín | Auth0 Token Vault, Arcade, AgentCore Identity [C5, C6] | PocketOS, Amazon Q | Không ngăn hành động qua công cụ của phiên bị chiếm (A15) | MEDIUM-HIGH |
| D1 tách luồng | CaMeL, FIDES | 2503.18813 Hình 9 | Role làm theo lời khách; mất tiện ích (CaMeL 77% so với 84%) | MEDIUM |
| Giao thức objective | OKR-Agent 2311.16542, contract-first 2602.11865, Contract Net, MetaGPT [C6 E1] | Loop-Back; Song; MSR | Không chặn bỏ lợi ích chặn trước khi gộp (2512.08296) | MEDIUM |
| Ngữ cảnh chung | Hearsay-II blackboard, 2510.01285, LangGraph, ETag [C6 E2] | ACE; 2607.04697 | Chưa đo trên tài liệu kế hoạch | MEDIUM |
| Checker xác định, số do mã | PAL, Program of Thoughts, data lineage [C6 E4] | FinSheet-Bench | Thiết kế và sản phẩm gần như không có checker [P §1.3, §1.4] | HIGH cho số |
| Panel đo n_eff | PoLL 2404.18796 [C6 E5] | Hossain, Kohli, 2604.07650 | Đo trên NLI, preference; n_eff vẫn khoảng 2–3,5 | MEDIUM |
| Tập neo, lỗi gieo niêm | Reusable holdout, Ladder (1411.2664, 1502.04585) | 2606.15474 | Một tác giả, một sự kiện đổi phiên bản | MEDIUM |
| pass^k, quy trình cùng kết quả | τ-bench, Rabanser, CXBench | 2406.12045, 2602.16666, 2610.25954 | τ đang bão hoà | HIGH cho hiện tượng |
| Surrogate, nhân quả quan sát | Athey, synthetic control, CausalImpact [C6 §4] | 2608.08043; 2607.01904 (staggered DiD) | Không thí nghiệm thì yếu hơn; cần đơn vị so sánh | LOW-MEDIUM |
| Owner hẹp, hàng đợi ngoại lệ | Chan et al. 2401.13138, Shavit (automatic monitoring) [C6 E6] | Kale, Vasconcelos, Green, Ye [R §2] | Ngắn hạn; n = 1; merge đặt owner vào vai người duyệt mỗi thay đổi | MEDIUM |
| Dừng | Perez, *The Law of Stop*, 2609.22882 [C6]: dừng ở một điểm có thể để hoạt động chạy ở chỗ khác | Palisade 2509.14260 | Hiện thực tham chiếu chưa đạt (mục 3) | MEDIUM |
| Release cho prompt và harness | SRE canary; hậu kiểm Anthropic | 2408.04667 [P §4] | Kiểm release không chặn (trừ merge) | LOW cho đủ |

## 8. Cố ý không đặc tả

Model và nhà cung cấp cụ thể (bảng xếp hạng đổi theo tháng); giá trị ngưỡng (không nguồn nào cho giá trị; công bố trước ngày 0 và niêm theo R2);
danh sách role; north-star và surrogate cụ thể; tuân thủ chi tiết theo nước (cần luật sư; [C4 §5] có 12 câu hỏi); huấn luyện model; giao thức
giữa các ranh giới; bố cục giao diện; ma trận kiểm soát (NIST AI RMF, ISO/IEC 42001, COSO, SR 26-2) và chứng nhận (SOC 2, ISAE 3402): cần
cho người mua [C5 §5], là sản phẩm riêng của hội đồng, không phải phần này.

## 9. Theo dõi gì khi vận hành và sửa tại chỗ ra sao

Không nhánh đối chứng. Số đo là quan sát (trace, verdict, ledger, phản hồi khách), kiểm tra dụng cụ (lỗi gieo, dữ liệu phát lại, lặp tác vụ
ghi lại) hoặc so sánh với đơn vị ngoài ranh giới. Đọc lần đầu sau 8–12 tuần vì lợi ích đầu kỳ tan sau khoảng hai tháng (2511.04427), và đọc
lại sau ít nhất 10 tháng vì thông lượng ở 2607.01904 chỉ đạt 2× ở tháng thứ mười [C2 §7]. Sửa tại chỗ là đổi brief, playbook, checker hoặc
mã qua release (I12).

| # | Theo dõi | Đo | Sửa tại chỗ |
|---|---|---|---|
| W1 | Thời gian phát hiện, dừng, khôi phục theo lớp hiệu ứng | Lỗi gieo; diễn tập ở mạng host | Thêm tripwire; rút đường dừng |
| W2 | Số agent so với việc phân rã được | Lượt suy luận, thành công trên 1.000 token theo cell | Brief của Chair |
| W3 | Finding và key result `unverifiable` | Tỷ lệ sửa, phản bác, ngoại lệ; tỷ lệ `unverifiable` theo thời gian | Brief contract-first; tập `reason_code` |
| W4 | Mục tiêu tự đặt bị hạ | (mục tiêu − kết quả) / kết quả theo role | Brief đặt mục tiêu; kiểm mẫu |
| W5 | Tỷ lệ bắt lỗi của checker, auditor, Chair | Lỗi gieo, kèm báo động nhầm và d′ | Thêm checker; đổi panel |
| W6 | Độc lập của panel trên dữ liệu thật | ρ, n_eff trên tập neo niêm | n_eff gần 1 thì dồn sang checker |
| W7 | Quá khớp với bộ kiểm | Tỷ lệ đạt hồi quy so với khiếu nại và lỗi thoát 30 ngày | Làm mới phần niêm của bộ hồi quy |
| W8 | Hành động H2 có tham số từ nội dung ngoài | I19, D6 | Mở rộng D1; nêu lại D4 với owner |
| W9 | Owner bỏ sót | Lỗi tìm thấy sau merge; tỷ lệ merge không bình luận; tuổi p95 của việc chờ | Xếp hạng ngoại lệ; bằng chứng ngắn hơn |
| W10 | Surrogate trôi | Lệch dấu với kết quả dài hạn; DiD với đơn vị ngoài ranh giới | Đưa về `expired` |
| W11 | Suy giảm sau release | Phân phối theo `config_hash`; báo lỗi của khách | Revert theo băm |
| W12 | Nợ và tải | Cảnh báo tĩnh, độ phức tạp, sự cố mỗi thay đổi | Chain dọn nợ |

## 10. Giả định và điểm yếu đã biết

Giả định, nguyên văn:

- "Assumption: một biện pháp là giới hạn nếu nó khiến một hành động agent đã chọn không xảy ra hoặc phải chờ một chủ thể khác; đổi kênh, nhãn
  hay khả năng hoàn tác không là giới hạn, theo định nghĩa của challenge-03 §1."
- "Assumption: đo trước ngày 0 trên tác vụ ghi lại (tỷ lệ bắt, thời gian phát hiện) là quan sát theo R3 và không là giai đoạn, vì không hành
  động nào của agent phải chờ nó."
- "Assumption: so sánh quan sát với đơn vị ngoài ranh giới (DiD, synthetic control) không phải thí nghiệm theo R3, vì không có phân nhóm ngẫu
  nhiên, holdout hay A/B trên khách."
- "Assumption: bản sao lưu và log nằm ngoài mọi công cụ của agent là trạng thái do server giữ, không phải giới hạn sáng kiến; không có chúng
  thì 'sửa khi chạy' không làm được sau một lần xoá."
- "Assumption: một công ty vận hành bằng agent luôn có ít nhất một người đại diện là thể nhân (Delaware DGCL §141(b); Luật Doanh nghiệp 2020
  Điều 12(1)), và công ty chịu trách nhiệm cho lời của agent [R §1.1]."
- "Assumption: các nghĩa vụ luật ở mục 2.7 đọc theo challenge-04 (toàn văn từ cơ sở dữ liệu, chưa đối chiếu Công báo); đây không phải tư vấn
  pháp lý."
- "Assumption: Remit là hiện thực tham chiếu; ADR và mã của nó chứng minh control plane hiện thực được ở mức đã có, không chứng minh nó
  hiệu quả, và các khoảng trống ở mục 3 là yêu cầu chưa có."

Điểm yếu tự nhận:

1. Lớp H2 không có cận; với D4 mặc định tắt, không biện pháp nào đứng trước một email, một khoản chuyển tiền hay một lần lộ dữ liệu ngoài D1.
2. D1 không bảo vệ role mà việc chính là làm theo lời khách; đó là role hỗ trợ khách, nơi injection gián tiếp mạnh nhất (2507.20526).
3. Không thí nghiệm và bật cùng lúc trong ranh giới: nhân quả chỉ có từ đơn vị ngoài ranh giới, không ngẫu nhiên, chịu khác biệt giữa đơn vị.
4. Merge do owner không trần (nếu owner bỏ trần ADR-0001) đặt owner vào vai người duyệt mỗi thay đổi; hàng đợi có thể sập (52 MR chờ khi đo,
   ADR-0002:40).
5. Hiện thực tham chiếu chưa thoả I1, I2, I5 (mục 3); mọi lời hứa về dừng và credential là yêu cầu cho tới khi diễn tập đo được.
