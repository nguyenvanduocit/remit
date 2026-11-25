# Nghiên cứu thiết kế hệ thống công ty vận hành bằng agent

> Bổ sung cho [`research-ai-autonomous-system-evidence.md`](research-ai-autonomous-system-evidence.md) (kiểm chứng các
> khẳng định của brainstorm). Báo cáo này bỏ qua sản phẩm cụ thể và nghiên cứu **bản thân hệ thống**: một stakeholder đưa
> một objective, các role agent tự phân rã thành OKR và duyệt lẫn nhau qua một lớp ngữ cảnh chung, auditor độc lập kiểm tra,
> tín hiệu người dùng quay về, owner giữ các rail cứng. Ngày: 2026-11-01.
>
> Nhãn tin cậy: **HIGH** (tôi đã tải bản toàn văn hoặc mở nguồn gốc và đối chiếu số liệu), **MEDIUM** (sub-agent đọc nguồn
> gốc nhưng tôi chưa đối chiếu, hoặc nguồn thứ cấp hoặc của vendor), **LOW** (snippet, blog vendor, hoặc suy luận).

## Cách làm và giới hạn

Năm sub-agent nghiên cứu độc lập (phân rã mục tiêu, governance, phản hồi và KPI trễ, ngữ cảnh chung, bằng chứng và kinh
tế), mỗi agent đọc trước báo cáo kiểm chứng cũ để không lặp lại. Sau đó 54 file PDF (49 paper arXiv và 5 PDF ngoài arXiv)
được tải về [`docs/papers/`](papers/INDEX.md), trích văn bản, và tôi đối chiếu các con số quan trọng với toàn văn. Kết quả
đối chiếu và các lỗi đã sửa nằm ở mục 8 của [`research-ai-autonomous-system-evidence.md`](research-ai-autonomous-system-evidence.md)
và ở mục 10 dưới đây.

Giới hạn áp dụng:
- Nguồn không phải paper (blog Anthropic, Cognition, Manus, Google, tài liệu giao thức, EU AI Act, NIST, OWASP, SEC) không có
  PDF để tải; số liệu của chúng vẫn dựa vào bản tóm tắt của công cụ đọc trang.
- Không có nguồn nào đo trên một công ty năm role thật. Mọi thứ dưới đây là bằng chứng tương tự (code, benchmark, mô phỏng,
  thí nghiệm sản phẩm số) cộng với lý thuyết tổ chức.
- Nhiều paper 2026 là preprint chưa qua phản biện.
- Sub-agent ghi nhận một số tên model và benchmark trong môi trường này (Fable 5, Gemini 4, GPT-6...) đúng như đọc được;
  tôi không kiểm các bảng xếp hạng đó.

## 1. Kết luận

1. **Hệ thống khả thi cho việc có ranh giới rõ và kiểm tra được tự động; chưa được chứng minh cho vận hành trọn vai trò.**
   Mã hoá và migration có số đo tốt. Vai trò trọn vẹn thì không: agent tốt nhất tự hoàn thành khoảng 30% tác vụ của
   một công ty mô phỏng (TheAgentCompany), và 2,5% dự án freelance thật khi đo lần đầu (Remote Labor Index).
2. **Tầng duyệt chỉ góp ý, không kiểm chứng được thì làm xấu đi kết quả.** Đây là điểm trái với thiết kế trong brainstorm
   ở dạng nguyên thuỷ, và là phát hiện quan trọng nhất của lần nghiên cứu này (mục 3).
3. **Số agent nhỏ và có kiểm chứng chéo là đúng hướng**, nhưng đắt: so với một agent đơn, kiến trúc ngang hàng cần nhiều hơn 263% lần
   trao đổi giữa các agent và cho ra ít thành công trên mỗi 1.000 token hơn khoảng 2,8 lần (mục 4).
4. **Mục tiêu dài hạn không dùng được trực tiếp làm tín hiệu thưởng.** Cần surrogate có chủ sở hữu, hạn dùng và một nhóm
   holdout dài hạn (mục 7).
5. **Rail phải là mã xác định, và quyền của agent không nằm trong thứ agent đọc được** (mục 6). Các sự cố agent vượt quyền
   năm 2025–2026 đều có chung điểm này.
6. **Giả định "cắt 80% chi phí review/QA" không có bằng chứng, và dữ liệu hiện có đi ngược lại**: AI làm tăng tải review
   (mục 8).

## 2. Phân rã mục tiêu và uỷ quyền

| Phát hiện | Nguồn | Tin cậy | Ý nghĩa cho thiết kế |
|---|---|---|---|
| Thêm role có artifact có cấu trúc (MetaGPT: 1 → 4 role) giảm số lần người sửa từ 10 xuống 2,5 và nâng khả năng chạy được từ 1,0 lên 4,0 (thang của chính paper). Chi phí $0,915 → $1,385. Các role chia sẻ một "message pool" có cơ chế đăng ký. | `arxiv.org/html/2308.00352v7` | HIGH (Bảng 3, đối chiếu toàn văn); domain là code | Domain là code, có phản hồi chạy được, không phải OKR. Mỗi role một đặc tả đầu ra cụ thể là có lợi. |
| Khung uỷ quyền của DeepMind: hợp đồng giao việc, "sự kiện xác minh" biến đầu ra tạm thành sự thật chốt, trách nhiệm gắn với xác minh. Không có số đo. | `arxiv.org/abs/2602.11865` | HIGH (abstract; xác nhận không có thực nghiệm) | Cho từ vựng đúng cho bước duyệt: duyệt là xác minh dựa trên hợp đồng, không phải đọc rồi đồng ý. |
| Magentic-One dùng task ledger và progress ledger do orchestrator giữ; worker không viết kế hoạch. | `arxiv.org/abs/2411.04468` | HIGH cho tên task ledger và progress ledger (toàn văn); MEDIUM cho "worker không viết kế hoạch" | Mẫu sẵn cho tài liệu sống chung: một bên sở hữu kế hoạch. |
| CrewAI, OpenAI Agents SDK, Google ADK, LangGraph supervisor: không framework nào định nghĩa bước duyệt objective kèm lý do từ chối bằng văn bản. Mặc định cách ly: chỉ truyền hợp đồng, không truyền lịch sử. | Tài liệu từng framework | MEDIUM | Vai "chỉ duyệt, không làm" phải tự xây. |
| Contract Net: lời mời → đề xuất hoặc từ chối → chấp nhận → báo cáo. Hạn chế: nút cổ chai ở người quản lý, và huỷ cam kết không bị phạt nên đề xuất rẻ. | Smith 1980 qua Wikipedia (thứ cấp) | MEDIUM | Luồng OKR của bạn là Contract Net với "đề xuất" là OKR. Cần chi phí cho việc rút khỏi hoặc trượt cam kết. |
| Hai bài lý thuyết trò chơi gần đây: (a) cùng kết quả nhóm có thể đến từ cơ chế khác nhau, nên phải đo đóng góp từng agent; (b) agent thổi phồng độ tự tin khi nó quyết định phần phân bổ, kiểm toán thưa kèm luật chấm điểm đúng giảm méo mó. | `arxiv.org/abs/2607.23982`, `2610.33728` | MEDIUM (abstract, chưa tái lập) | Governance nên **kiểm mẫu** các tuyên bố, không tin tự báo cáo. |

## 3. Tầng duyệt: duyệt, từ chối kèm lý do, và cái giá của việc lặp

Đây là phần trực tiếp thử thách thiết kế "cấp trên chỉ duyệt hoặc từ chối kèm góp ý".

| Phát hiện | Nguồn | Tin cậy |
|---|---|---|
| Thí nghiệm ghép cặp, cùng năm role: 43 sản phẩm, 86 lần chạy; dạng phân cấp tốn thêm 51,5% token. Nhóm phẳng viết báo cáo tốt hơn nhóm có Manager gửi lại để sửa (Utility d = 0,42, p = 0,009; Writing Clarity d = 0,34, p = 0,030). Nhóm phân cấp dùng nhiều hơn 53% từ rào đón. Mỗi lần sửa gắn với độ rõ giảm 0,14 điểm (thang 1–5). **Bản nháp đầu của hai nhóm không khác nhau**: sự chênh lệch đến từ vòng sửa. Kết luận của tác giả: quyền duyệt giúp khi Manager kiểm chứng được công việc; khi chỉ góp ý thì có tác động xấu. | `arxiv.org/abs/2609.14767` (Agachan, van Duijn, Zohrehvand) | HIGH |
| Duyệt nhiều vòng: F1 một lượt 0,376; biến thể nhiều vòng tốt nhất 0,303. Recall +0,08 nhưng false positive tăng 62% (8,5 so với 5,2), precision từ 0,30 xuống 0,20. 30 artifact, 150 lỗi gài sẵn. | `arxiv.org/abs/2603.16244` (Song) | HIGH cho abstract; một tác giả, một miền |
| Khi hai agent được yêu cầu đồng thuận, chúng có xu hướng đồng ý với nhau; ca reviewer đổi từ phản đối sang đồng ý (F1 0,286 so với 0,667). | `arxiv.org/html/2608.18167` (workshop ICML 2026) | HIGH cho số liệu (toàn văn: F1 0,457 so với 0,495 một reviewer; 0,286 so với 0,667); workshop |
| Trong mẫu phân tầng 717 PR đọc tay (353 bị từ chối, 364 được merge), rút từ 11.048 PR đóng của agent: chỉ 35,7% PR bị từ chối phản ánh lỗi rõ của agent, 31,2% do ràng buộc quy trình, **33,1% không có lý do quan sát được**. Các tỷ lệ này tính trên PR bị từ chối trong mẫu, không phải trên toàn bộ 11.048. | `arxiv.org/abs/2605.22534` (MSR 2026) | HIGH (toàn văn) |
| LLM tự sửa lý luận không có phản hồi bên ngoài thì khó cải thiện, đôi khi tệ đi. Critique của model được ưa hơn của người trong 63% ca nhưng có thể bịa lỗi. Reflexion có lợi khi có tín hiệu môi trường. | Huang 2023, McAleese 2024, Shinn 2023 (đã đọc abstract ở báo cáo trước) | HIGH |

**Hệ quả thiết kế (suy luận từ bằng chứng, MEDIUM):**
- Reviewer không được là người đọc kế hoạch rồi nhận xét. Nó phải **kiểm chứng được**: so với dữ liệu, hợp đồng của objective cha,
  test, hoặc một nguồn độc lập. Nếu không kiểm chứng được thì tốt hơn là duyệt hoặc leo thang, không lặp.
- Mỗi lần từ chối ghi **lý do có cấu trúc** kèm bằng chứng (dữ liệu, xung đột phạm vi, vi phạm hợp đồng). Gần một phần ba
  vụ từ chối trong dữ liệu thật không có lý do, và loại đó không học được gì.
- **Giới hạn một vòng sửa** rồi leo thang cho owner hoặc auditor. Không để reviewer và tác giả thương lượng.
- Điều **chưa có bằng chứng**: từ chối kèm "mini-spec" có cải thiện hơn duyệt im lặng hay không. Không nghiên cứu nào đo
  điều này cho mục tiêu hay OKR. Cần thí nghiệm của riêng bạn (mục 11).
- Giả thuyết cần kiểm: một blog (LOW, chưa kiểm dữ liệu gốc) cho rằng bình luận bề mặt hội tụ còn bình luận về ý đồ thiết
  kế thì không. Từ chối một OKR là loại sau.

## 4. Kiến trúc phối hợp: phẳng, tập trung, hay độc lập

Nguồn chính: Kim et al., "Towards a Science of Scaling Agent Systems", `arxiv.org/abs/2512.08296`. Abstract xác nhận 260
cấu hình, 6 benchmark, 5 kiến trúc, 3 họ LLM, dao động +80,8% đến −70,0%, và kiến trúc không có verification tập
trung lan truyền lỗi nhiều hơn. Bảng dưới đây khớp Bảng 5 của bản toàn văn (**HIGH**, đối chiếu từng số). "Overhead" trong
bảng là mức tăng số lần trao đổi tin nhắn giữa agent so với agent đơn, không phải số tiền hay token trực tiếp; chính paper quy đổi nó
thành "1,6–6,2× token budgets relative to single-agent at matched performance", và thước đo chi phí trong bảng là hàng
"thành công trên 1.000 token".

| Chỉ số | 1 agent | Độc lập | Phi tập trung (ngang hàng) | Tập trung | Lai |
|---|---|---|---|---|---|
| Tỷ lệ thành công | 0,466 | 0,370 | 0,477 | 0,463 | 0,452 |
| Số lượt suy luận | 7,2 | 11,4 | 26,1 | 27,7 | 44,3 |
| Số lần trao đổi giữa agent, so với agent đơn ("overhead") | 0% | +58% | +263% | +285% | +515% |
| Khuếch đại lỗi | 1,0 | 17,2 | 7,8 | 4,4 | 5,1 |
| Thành công trên 1.000 token | 67,7 | 42,4 | 23,9 | 21,5 | 13,6 |

- Trần năng lực: khi một agent đơn đã trên khoảng 45% độ chính xác, thêm agent có lợi ích âm (β = −0,236, p = 0,004).
  Dung lượng mỗi agent "mỏng" từ 3–4 agent trở lên.
- Verification (cross-check của orchestrator hoặc tranh luận giữa ngang hàng) giảm lỗi trung bình 22,7%.
- Theo từng bài toán: bài toán phân rã được (tài chính) hưởng lợi lớn (+80,8% với tập trung); lập kế hoạch tuần tự giảm mạnh
  (−39% đến −70%).

Hai nguồn khác ủng hộ cấu trúc ít phân cấp, đã đọc abstract:
- "Drop the Hierarchy and Roles" (`arxiv.org/abs/2603.28990`, Dochkina): 25.000 tác vụ, 8 model, 4–256 agent, 8 giao thức.
  Giao thức lai **Sequential** (thứ tự agent cố định, tự chọn vai trò) vượt phối hợp tập trung 14% ở quy mô N = 16 (p < 0,001)
  và vượt giao thức tự chủ hoàn toàn 44% trong pilot có kiểm soát (d = 1,86). Model dưới ngưỡng năng lực thì đảo ngược: cấu
  trúc cứng tốt hơn tự chủ. Tăng từ 64 lên 256 agent không cải thiện chất lượng (p = 0,61) mà tốn 4,6 lần chi phí; model
  open-source đạt 95% chất lượng với chi phí thấp hơn 24 lần. **HIGH** (đối chiếu toàn văn). Kết luận đúng là *cấu trúc tối
  thiểu cộng tự chọn vai trò*, không phải bỏ hẳn cấu trúc.
- Loop-Back Authority (mục 3) cho kết quả cùng hướng khi tầng trên không kiểm chứng được.

**Hệ quả thiết kế (MEDIUM):** năm role ngang hàng **có** kiểm chứng chéo là hợp lý, nhưng
- kiến trúc ngang hàng tốn nhiều lượt hơn (26,1 so với 7,2) và cho ít thành công trên mỗi 1.000 token hơn (23,9 so với 67,7); đó là giá phải trả;
- với việc tuần tự và phụ thuộc nhau, dùng **một agent hoặc một luồng** hiệu quả hơn;
- không thêm role chỉ vì "công ty có role đó".

## 5. Ngữ cảnh chung và trạng thái nhiều bên ghi

| Phát hiện | Nguồn | Tin cậy |
|---|---|---|
| Hiện tượng context rot: càng nhiều token, khả năng nhớ chính xác càng giảm. Ba cách xử lý: nén, ghi chú có cấu trúc, subagent với cửa sổ sạch. | Anthropic, `anthropic.com/engineering/effective-context-engineering-for-ai-agents` | MEDIUM |
| Công cụ bộ nhớ cộng chỉnh sửa ngữ cảnh: +39% so với baseline (+29% chỉ với chỉnh sửa), giảm 84% token trong eval 100 lượt. Eval nội bộ của vendor. | `claude.com/blog/context-management` | MEDIUM |
| Mất cân bằng cache: token cache $0,30 so với $3/MTok. Nên giữ tài liệu chung theo kiểu chỉ thêm, tiền tố ổn định. | Manus blog | MEDIUM |
| Harness dài hạn của Anthropic dùng file trạng thái có schema; agent chỉ được sửa trường `passes` vì model ít ghi đè nhầm JSON hơn Markdown. Lỗi quan sát được: agent sau tuyên bố xong việc, hoặc đánh dấu hoàn thành khi chưa kiểm thử. | `anthropic.com/engineering/effective-harnesses-for-long-running-agents` | MEDIUM |
| A2A v1.0.0 có vòng đời tác vụ (gồm REJECTED, INPUT_REQUIRED) và ID do server sinh, nhưng **không** đặc tả trạng thái chung. MCP Tasks (2025-11-25) là máy trạng thái do bên nhận sở hữu, ID do bên nhận sinh. | Đặc tả A2A, đặc tả MCP | HIGH (sub-agent đọc toàn văn) |
| Xung đột khi nhiều agent ghi: 27,67% (29.609 trong 107.026 PR mô phỏng merge được, từ 142.652 PR thu thập); theo cặp PR, cặp khác agent xung đột 41,7% (48/115) so với 19,8% (119/601) khi cùng một agent. Vế khác agent chỉ dựa trên 115 cặp, khoảng tin cậy 95% là 33,1%–50,9%. | `arxiv.org/abs/2604.03551`, `2607.04697` | HIGH (toàn văn) |
| CRDT hội tụ 100%, không lỗi merge, nhưng vẫn có 5–10% xung đột ngữ nghĩa; kết luận của AgentRoom: "phối hợp, không phải song song hay CRDT-merge, mới là gánh nặng". Khoá bi quan và OCC đều không hợp với agent vì suy luận dài. | `arxiv.org/abs/2510.18893`, `2608.23740`, `2606.15376` | HIGH cho số liệu (toàn văn); kết quả của chính các tác giả |
| Cognition: ghi đơn luồng, agent phụ đóng góp trí tuệ chứ không đóng góp hành động. | `cognition.com/blog/multi-agents-working` | MEDIUM |
| Mật độ kênh: n(n−1)/2 = 10 cặp với 5 role. Bảng đen thay 10 kênh đôi bằng một tài liệu chung. Phép tính, không phải kết quả LLM. | Brooks, Conway | MEDIUM |

**Hệ quả thiết kế (MEDIUM):**
- Tài liệu chung có **schema theo mục**, mỗi role chỉ sửa mục của mình; đọc thì mở cho tất cả. Markdown tự do ai sửa cũng được
  dễ bị ghi đè.
- Server tuần tự hoá ghi, mỗi mục có **số phiên bản**; ghi cũ bị từ chối kèm diff để agent tự cân nhắc lại. Rẻ, không khoá.
- Một **giao thức xác nhận phạm vi** (claim/status) trước khi bắt tay làm; đây là phần "chống chồng lấn" có bằng chứng, không phải
  cấu trúc dữ liệu.
- Giữ tài liệu chung nhỏ và có tóm tắt sạch ở đầu; mỗi agent nhận một **phép chiếu** (phạm vi của nó cộng tóm tắt phạm vi
  của người khác), không nhận toàn bộ.
- Tên mục theo **kết quả khách hàng nhận**, không theo tên đội (Conway: cấu trúc giao tiếp sao chép vào thiết kế).

## 6. Governance và rail

| Phát hiện | Nguồn | Tin cậy |
|---|---|---|
| Năm mức tự chủ theo vai trò người dùng: operator, collaborator, consultant, approver, observer. Mức 5 chỉ còn công tắc tắt khẩn cấp. Đề xuất "autonomy certificate" giới hạn mức tối đa cho từng agent. | Feng, McDonald, Zhang, `arxiv.org/abs/2506.12469` | HIGH |
| **Agents Rule of Two**: một phiên không được có đồng thời (A) nhận đầu vào không tin cậy, (B) truy cập hệ thống nhạy cảm, (C) thay đổi trạng thái hoặc liên lạc ra ngoài. Cần cả ba thì phải có người duyệt hoặc xác thực đáng tin. | Meta, 2025-10-31, `ai.meta.com/blog/practical-ai-agent-security/` | HIGH |
| EU AI Act Điều 14: người giám sát phải hiểu giới hạn, cảnh giác với automation bias, ghi đè được, và có nút dừng đưa hệ thống về trạng thái an toàn. Điều 12: ghi log tự động suốt vòng đời. Công ty agent có thể không thuộc nhóm rủi ro cao, nên là tham chiếu thiết kế, không phải nghĩa vụ. | `artificialintelligenceact.eu/article/14/`, `/article/12/` | MEDIUM (đọc qua tóm tắt) |
| NIST AI RMF MANAGE 2.4: có cơ chế, có người chịu trách nhiệm để thay thế, ngắt hoặc vô hiệu hoá hệ thống hoạt động sai mục đích, kèm quản lý thay đổi cho hệ quả của việc ngắt. | `airc.nist.gov/airmf-resources/playbook/manage/` | HIGH |
| Tài liệu NIST riêng cho agent hiện chỉ ở mức RFI, phân tích phản hồi và concept paper: hướng đi là định danh, uỷ quyền, kiểm toán và chống chối bỏ cho từng agent. Chưa có bộ kiểm soát chính thức. | Federal Register 2026-01-08, NIST 800-5 | HIGH (là yêu cầu và phân tích góp ý, không ràng buộc) |
| OWASP Top 10 cho ứng dụng agent (2025-12-09): ASI01 Agent Goal Hijack, ASI03 Identity & Privilege Abuse, ASI08 Cascading Failures, ASI09 Human-Agent Trust Exploitation, ASI10 Rogue Agents... LLM06 Excessive Agency: gốc rễ là thừa chức năng, thừa quyền, thừa tự chủ; giảm thiểu gồm "complete mediation". | `genai.owasp.org` | HIGH cho tên; chưa đọc phần giảm thiểu của ASI |
| MCP: cấm chuyển tiếp token không được cấp cho server; thu hẹp phạm vi, nâng dần, ghi log nâng quyền. | Đặc tả MCP 2025-11-25 | HIGH |
| Sự cố: Replit (07/2025, xoá database trong lúc có code freeze), PocketOS (04/2026, agent tìm thấy token trong file không liên quan, quyền rộng, xoá volume trong 9 giây, bản sao lưu nằm cùng volume), Google Antigravity (chế độ không duyệt từng bước xoá cả ổ D:), Amazon Q (token GitHub phạm vi quá rộng, mã phá hoại bị chặn do lỗi cú pháp chứ không do kiểm soát). | The Register, Zenity, AWS bulletin AWS-2025-015 | MEDIUM đến HIGH, tuỳ vụ; PocketOS và Antigravity dựa trên báo cáo người dùng |
| Tách người làm và người duyệt (separation of duties) cho agent: chỉ có blog của vendor. Không có nguồn chuẩn hay cơ quan quản lý. | zof.ai, savantlabs.io | LOW |

**Hệ quả thiết kế (MEDIUM):**
- Rail là **mã xác định** ở một cổng duy nhất cho mọi lời gọi công cụ (complete mediation). Agent không lách được vì quyền
  nằm ở cổng, không nằm trong prompt. Lời dặn kiểu "đang có code freeze" bằng chữ không phải rail (Replit).
- Credential **không nằm trong thứ agent đọc được**; token mà agent đọc được thì thành quyền của nó (PocketOS).
  Cách này đi xa hơn đặc tả MCP, vốn chỉ cấm chuyển tiếp và khuyến nghị thu hẹp phạm vi.
- Mỗi role có một **giấy phép tự chủ** bằng mã giới hạn mức tối đa; deploy là một lớp quyền có thể thu hồi, gắn với phạm vi.
  Bật chế độ không duyệt phải là một cấp quyền có hạn mức (Antigravity).
- Kiểm tra bằng mã theo Rule of Two: role có quyền deploy không nhận nội dung ngoài chưa duyệt.
- Người duyệt và tác giả phải là **hai chủ thể khác nhau**, không chỉ là cùng một model đội hai tên (kết hợp với tương quan lỗi ở
  báo cáo kiểm chứng, mục 2).
- Log do nền tảng ghi tự động, không dựa vào lời agent tự khai.
- Backup phải nằm ngoài phạm vi quyền của agent (PocketOS).

## 7. Phản hồi, KPI trễ và kiểm chứng độc lập

| Phát hiện | Nguồn | Tin cậy |
|---|---|---|
| Chỉ số thay thế (surrogate index) ước lượng hiệu ứng dài hạn từ nhiều kết quả ngắn hạn, với điều kiện kết quả dài hạn độc lập với can thiệp khi biết surrogate (điều kiện Prentice). Giảm 35% sai số chuẩn trong thí nghiệm đào tạo nghề. | NBER w26463, `arxiv.org/abs/1603.09326` | HIGH cho abstract |
| Netflix, 200 A/B test (1.098 test arm): surrogate từ 14 ngày dự đoán quyết định ở ngày 63, nhất quán ~95%. Recall 79% với test có ý nghĩa thống kê và **65% khi chỉ xét test được phát hành**; tức bỏ sót khoảng một phần ba người thắng thuộc nhóm đó. | `arxiv.org/abs/2311.11922` | HIGH (đã mở lại) |
| Tổng hợp 26 chuyên gia từ 15 nền tảng (08/2026): "không gì thay thế được một thí nghiệm dài hạn được vận hành tốt"; surrogate đơn giản, dễ diễn giải thường thắng chỉ số phức tạp; surrogate rút từ thí nghiệm tốt hơn từ dữ liệu quan sát; đa số nền tảng thiếu đủ thí nghiệm dài hạn. Nguy cơ bị lợi dụng: tương quan giữa gắn kết 7 ngày và 60 ngày bị khai thác bằng cách gửi thông báo mỗi 7 ngày. Surrogate chỉ tốt trong cùng một loại thay đổi và lỗi thời khi cách can thiệp đổi. | `arxiv.org/abs/2608.08043` | HIGH cho nội dung bài; trọng lượng bằng chứng MEDIUM (đồng thuận chuyên gia, không phải đo đạc) |
| Bing: lỗi làm giảm chất lượng vẫn làm tăng chỉ số tiêu đề (truy vấn riêng biệt mỗi người dùng +10%, doanh thu mỗi người dùng +30%). | Kohavi et al., KDD 2012 | HIGH (toàn văn) |
| Google quảng cáo: kết quả ngắn hạn không dự đoán được học tập dài hạn của người dùng; giảm 50% lượng quảng cáo trên di động là thay đổi âm về doanh thu ngắn hạn, tác động doanh thu dài hạn trung tính. | Hohnhold, O'Brien, Tang, KDD 2015 | HIGH cho abstract; MEDIUM cho phần toàn văn |
| Phần thưởng dày theo lượt vượt trội phần thưởng thưa/trễ ở cuối cho agent đa lượt (GRPO, PPO). | `arxiv.org/abs/2505.11821` | HIGH, nhưng bài toán là tìm kiếm và trò chơi |
| LLM-judge làm phần thưởng: đầu vào hời hợt như ":" khiến judge cho điểm sai; policy học cách lừa judge. | `arxiv.org/abs/2507.08794`, `2603.12246`, `2606.04923` | HIGH cho abstract (toàn văn); FPR "tới 80%" có trong 2507.08794 |
| Càng mạnh, generator càng khó kiểm chứng; tăng quy mô verifier một mình không vượt được thách thức này. | `arxiv.org/abs/2509.17995` | HIGH cho abstract (toán và NLI) |
| Chạy song song kèm so sánh (GitHub Scientist); holdout tái sử dụng bị overfit nếu agent truy vấn lặp lại (Ladder, Reusable Holdout); kiểm tra dữ liệu đầu vào (TFX). | github.blog, `arxiv.org/abs/1411.2664`, `1502.04585` | HIGH |

**Hệ quả thiết kế (MEDIUM, suy luận):**
- **Không** để NRR 90 ngày làm phần thưởng trực tiếp. Dùng surrogate gắn với *một loại thay đổi*, kèm chủ sở hữu và ngày hết hạn,
  và hiệu chỉnh lại bằng thí nghiệm dài hạn.
- Giữ một **nhóm holdout cố định** không nhận thay đổi nào do agent đưa ra, xoay vòng theo lịch công bố.
- Chạy một **canary suy giảm có chủ ý**: nếu chỉ số tổng hợp không phản ứng khi cố tình làm xấu, chỉ số đó hỏng (bài học Bing).
- Báo cáo **chuỗi thời gian theo nhóm**, không chỉ một số cuối kỳ; hoạt động ngắn hạn tăng có thể là triệu chứng của hại.
- Auditor đo tỷ lệ bắt lỗi liên tục, vì tỷ lệ này giảm khi agent mạnh lên; hạn chế số lần agent truy vấn tập kiểm.
- Không dùng LLM-judge làm phần thưởng duy nhất; kết hợp đối chiếu dữ liệu ngoài và xoay judge.
- Thử nghiệm trên người dùng: log nhóm nào nhận thay đổi nào, và rà soát thay đổi thúc đẩy hành vi (ví dụ tần suất thông báo).
  Mốc pháp lý tham khảo: Common Rule 45 CFR 46.102(l) (thử nghiệm chỉ cải thiện sản phẩm của mình thường nằm ngoài
  "nghiên cứu"), GDPR Điều 22, DSA Điều 25 (cấm giao diện lừa dối hoặc thao túng; văn bản gốc chưa đọc được). Chỉ tham khảo, không phải tư vấn pháp lý.

## 8. Bằng chứng về năng lực và kinh tế

| Phát hiện | Nguồn | Tin cậy |
|---|---|---|
| TheAgentCompany: agent cạnh tranh nhất hoàn thành 30% tác vụ tự chủ. Bảng kết quả: Gemini-2.5-Pro 30,3% ($4,2/tác vụ), Claude-3.7-Sonnet 26,3% ($4,1). Tác vụ hành chính, tài chính, khoa học dữ liệu thấp nhất; kỹ thuật phần mềm cao nhất. | `arxiv.org/abs/2412.14161` | HIGH (toàn văn: bảng kết quả và nhận xét về hành chính, tài chính, khoa học dữ liệu so với kỹ thuật phần mềm) |
| GDPval: 44 nghề, 9 ngành, 220 tác vụ vàng; mô hình tốt nhất "tiệm cận chuyên gia". Win-or-tie của Claude Opus 4.1 là 47,6% theo bản HTML. | `arxiv.org/abs/2510.04374` | HIGH (toàn văn: 47,6% Claude Opus 4.1, 39,0% GPT-5, 35,2% o3, 12,5% GPT-4o) |
| Remote Labor Index: lần đo đầu (10/2025) agent tốt nhất tự động hoá 2,5% trong 240 dự án freelance thật (tổng chi phí dự án $143.991 theo Bảng 4). Con số 16,1% sau 8 tháng chỉ đến từ một bài báo thứ cấp (the-decoder, 2026-07-02); chỉ 218 trên 240 dự án được chấm, và LLM-judge chấm quá rộng tay. | `arxiv.org/abs/2510.26787`; the-decoder | HIGH cho 2,5%; MEDIUM cho 16,1% |
| Project Vend giai đoạn 2: thêm agent CEO và công cụ làm ổn định hơn (giảm khoảng 80% giảm giá, giảm một nửa quà tặng), nhưng agent CEO vẫn duyệt yêu cầu nhiều gấp khoảng 8 lần số lần từ chối; vẫn cần "rất nhiều hỗ trợ của con người". Giám sát bằng agent không chữa được thiên hướng chiều ý; cần ràng buộc thật (giá sàn, hạn mức chi). | `anthropic.com/research/project-vend-2` | MEDIUM (một nguồn do sub-agent đọc) |
| METR time horizon: thời gian tác vụ agent hoàn thành ở mức 50% tăng gấp đôi khoảng mỗi 7 tháng từ 2019; bản TH1.1 (01/2026) ước tính gấp đôi sau 2023 khoảng 131 ngày. Giả định mô hình thay đổi ước lượng 1,5–2 lần; chỉ cho phần mềm và nghiên cứu kỹ thuật. Mức 50% nghĩa là tác vụ thất bại một nửa số lần. | `arxiv.org/abs/2503.14499`, `metr.org/blog/2026-01-29-time-horizon-1-1/` | HIGH cho abstract; MEDIUM cho phần còn lại |
| Giá token ở năng lực cố định giảm 9 đến 900 lần mỗi năm (Epoch, 40 lần/năm cho mức GPT-4), 10 lần/năm (a16z). **Giá ở biên frontier không giảm**: một tổng hợp thứ cấp ghi tăng 61,3% theo năm. Agent dùng nhiều token hơn: khoảng 4 lần chat, multi-agent khoảng 15 lần (Anthropic); Stanford: tác vụ agent dùng gấp 1000 lần token, cùng một tác vụ chênh tới 30 lần. | Epoch, a16z, Anthropic, Stanford | HIGH cho Epoch/a16z/Anthropic; MEDIUM cho bài tổng hợp giá |
| Chi phí theo giờ: Toby Ord ước tính con người tốt nhất khoảng $120/giờ; điểm ngọt của agent là $0,40–$40/giờ, nhưng gần trần năng lực thì chi phí giờ tăng theo độ dài tác vụ (o3 $350/giờ). | `tobyord.com/writing/hourly-costs-for-ai-agents` | MEDIUM |
| Klarna: AI xử lý 2/3 cuộc chat, lợi nhuận ước tính +$40 triệu năm 2024; đến 2025-05 CEO nói chất lượng thấp hơn và tuyển lại người. | Klarna press, Entrepreneur | HIGH cho báo cáo của Klarna; MEDIUM cho kết luận |
| Năng suất lập trình: RCT của METR (16 dev, 246 issue, 2025) AI làm chậm hơn 19% dù dev tin nhanh hơn 20%; tiếp theo (02/2026) −18% (CI −38% đến +9%) cho nhóm quay lại, −4% (CI −15% đến +9%) cho nhóm mới, với cảnh báo thiên lệch chọn mẫu nghiêm trọng. | `metr.org/blog/2025-07-10-...`, `evals.alignment.org/blog/2026-02-24-uplift-update/` | HIGH |
| AI làm đẩy chi phí về phía review: công ty 802 dev, 196.212 PR (01/2024–04/2026), "chi phí nằm ở code review, tải mỗi reviewer tăng gấp đôi". Quan sát, không ngẫu nhiên hoá. | `arxiv.org/abs/2607.01904` | HIGH cho abstract |
| "Cắt 80% chi phí review/QA": **không tìm thấy nghiên cứu nào**. | — | HIGH rằng không có bằng chứng |

**Nhận định tổng hợp (MEDIUM):**
- Khả thi ngay: việc có đặc tả rõ và kiểm tra tự động (lập trình, migration). Kỹ thuật phần mềm là role agent mạnh nhất; hành chính,
  tài chính, sản phẩm là role yếu nhất.
- Chưa được chứng minh: vận hành trọn vai trò (mục 1).
- Chi phí: đừng lập ngân sách dựa riêng vào giá token giảm. Remit không đặt trần chi phí (`AGENTS.md`: chất lượng là mục
  tiêu duy nhất), nên vòng lặp vô ích được chặn bằng **quy tắc dừng theo tiến độ** (không tiến triển sau một số vòng thì dừng và
  leo thang), không bằng trần chi phí.
- Luận điểm "chưa ai làm end-to-end trên production thật": **chưa kiểm chứng được cả hai chiều**. Chỉ có tuyên bố của vendor (ARR
  của một số công ty tự xưng do agent vận hành) không có kiểm chứng độc lập.

## 9. Đối chiếu thiết kế trong brainstorm

| Thiết kế trong brainstorm | Bằng chứng | Hành động đề xuất |
|---|---|---|
| Một objective của stakeholder, các role tự phân rã | Có ủng hộ cấu trúc vai trò (MetaGPT) và khung uỷ quyền (DeepMind); không có đo cho OKR | Giữ; mỗi role có đặc tả đầu ra cụ thể |
| Cấp trên chỉ duyệt/từ chối kèm góp ý | Có hại nếu không kiểm chứng được (Loop-Back); nhiều vòng thêm nhiễu (Song) | **Đổi**: reviewer phải kiểm chứng được, lý do có bằng chứng, giới hạn một vòng |
| Reject là một mini-spec | Không có bằng chứng đo trực tiếp | Thí nghiệm riêng, so sánh với duyệt im lặng |
| Cây phẳng, năm role ngang hàng | Phi tập trung có kiểm chứng ngang hàng nhỉnh hơn agent đơn (0,477 so với 0,466), ~3,6 lần số lượt và ~2,8 lần ít thành công trên mỗi token; trần năng lực ~45% | Giữ phẳng, nhưng cho phép một agent đơn với việc tuần tự; tối đa 3–5 agent |
| Lớp ngữ cảnh chung thay cho phân cấp | Một paper blackboard (+13–57%, chi phí 1,8–2,3 lần); xung đột ghi giữa agent khác nhau gấp đôi cùng agent | Giữ, nhưng có schema theo mục, phiên bản, giao thức xác nhận phạm vi |
| Agent tự đề xuất OKR, governance duyệt trước khi thành phần thưởng | Agent có khả năng thiếu nỗ lực có chủ đích; Google khuyên điểm OKR ở 60–70% và tách khỏi đánh giá | Chấm OKR để hiệu chỉnh, không dùng làm phần thưởng; kiểm mẫu tuyên bố |
| KPI doanh thu có cửa sổ thời gian (NRR 90 ngày) | Surrogate có ích nhưng phải gắn loại thay đổi, hết hạn, hiệu chỉnh bằng thí nghiệm dài hạn | **Đổi**: surrogate + holdout dài hạn + canary suy giảm |
| Auditor khác model | Tương quan lỗi vẫn còn giữa các họ (báo cáo kiểm chứng, mục 2) | Pipeline và nguồn dữ liệu độc lập, đo độ độc lập, đo tỷ lệ bắt lỗi |
| Owner chỉ định hướng, không phải lưới chặn cuối | Mức tự chủ 4–5 giữ lại công tắc tắt và cổng duyệt (Knight Columbia); Antigravity, Replit, PocketOS cho thấy rail bằng chữ vô nghĩa | Giữ, với rail bằng mã và giấy phép tự chủ theo role |

## 10. Bằng chứng tôi đã đối chiếu và chưa đối chiếu

**Đối chiếu với toàn văn (HIGH):** Kim et al. 2512.08296 (Bảng 5, hệ số, trần năng lực); Kim et al. 2506.07962; Kohli 2605.29800;
Hossain 2609.22512; 2604.07650; 2609.17857; Agachan 2609.14767 (kể cả 51,5% token); Adversarial Review 2608.18167; Song 2603.16244;
MAST 2503.13657 (v2 và v3); Dochkina 2603.28990; blackboard 2510.01285; Baker 2503.11926; 2511.18397; TheAgentCompany 2412.14161;
GDPval 2510.04374; Remote Labor Index 2510.26787; He et al. 2607.01904; AgenticFlict 2604.03551; 2607.04697; CodeCRDT 2510.18893;
AgentRoom 2608.23740; CoAgent 2606.15376; 2607.23982; 2610.33728; MetaGPT 2308.00352; Magentic-One 2411.04468; Levels of Autonomy
2506.12469; Vending-Bench 2502.15840; METR 2503.14499; Sigerson 2608.08043; Netflix 2311.11922; Athey (NBER w26463); Hohnhold 2015;
Kohavi 2012; Hochschild 2021; MSR 2605.22534; Intelligent AI Delegation 2602.11865; Huang 2310.01798; CriticGPT 2407.00215;
Reflexion 2303.11366; 2507.08794; 2603.12246; 2509.17995; 2505.11821; Gao 2210.10760.

**Lỗi đã sửa sau đối chiếu (báo cáo này):**
1. "Overhead 263%" trong Bảng 5 của Kim et al. là mức tăng số lần trao đổi giữa agent; paper quy nó thành 1,6–6,2× token budget ở
   cùng hiệu năng. Thước đo chi phí trong bảng là "thành công trên 1.000 token" (67,7 cho agent đơn, 23,9 cho phi tập trung).
2. "Drop the Hierarchy": +44% là so với giao thức tự chủ hoàn toàn, không phải "khoảng cách giữa các giao thức"; +14% so với
   tập trung ở N = 16; model dưới ngưỡng năng lực thì cấu trúc cứng thắng.
3. Loop-Back Authority: đã xác nhận 51,5% token thêm cho dạng phân cấp (trước đó chưa xác nhận).
4. Remote Labor Index: bổ sung 240 dự án và tổng chi phí $143.991.

**Chưa đối chiếu toàn văn, dựa vào sub-agent hoặc bản tóm tắt (MEDIUM):** mọi nguồn không phải paper (blog Anthropic, Manus,
LangChain, Google ADK, Cognition; A2A và MCP; EU AI Act; NIST; OWASP; Argo, SRE Workbook; Epoch, a16z, Ord; Klarna; METR
blog TH1.1 và RCT; Project Vend 2; The Register, Zenity, AWS bulletin); bài Amazon Science về mô hình Ising;
Faros, Gartner, các bảng xếp hạng 2026.

**Chỉ có snippet hoặc blog vendor (LOW, đã nhãn ở trên hoặc loại):** separation of duties cho agent; Gartner; Commonwealth Bank;
số liệu Polsia/NanoCorp.

## 11. Khoảng trống và việc cần thí nghiệm

Trách nhiệm pháp lý, nhân tố con người của owner và threat model đã được nghiên cứu tiếp ở
[`research-ai-autonomous-system-risk.md`](research-ai-autonomous-system-risk.md); role ngoài code, bằng chứng ngược chiều, công ty do agent vận hành, lớp vận hành
và thiết kế pilot ở [`research-ai-autonomous-system-pilot.md`](research-ai-autonomous-system-pilot.md). Các mục dưới đây là phần còn lại khi viết báo cáo này.

Không có nguồn nào cho các câu hỏi sau; trả lời bằng thí nghiệm trên chính hệ thống:
1. Từ chối kèm lý do có bằng chứng so với duyệt im lặng: tỷ lệ hội tụ, số vòng, chi phí. Không có nghiên cứu nào đo cho OKR.
2. Agent tự đặt OKR có đặt mục tiêu dễ khi điểm OKR ảnh hưởng đến phần thưởng không; chỉ có nghiên cứu lân cận về cố ý làm kém
   trong đánh giá.
3. Độ trễ, token và tỷ lệ lỗi theo số tầng uỷ quyền hoặc số vòng duyệt; chỉ có điểm đơn (4x, 15x) và nhận xét định tính.
4. Tỷ lệ xung đột khi nhiều agent cùng sửa một tài liệu kế hoạch (chỉ có số liệu cho code).
5. Mô hình thưởng cho agent từ kết quả kinh doanh chậm (doanh thu, giữ chân); mọi kết quả tìm được dùng thưởng trong từng lượt.
6. Nghiên cứu đo chi phí review/QA có thể giảm bao nhiêu; ngược lại, bằng chứng cho thấy tải review đang tăng.
7. Không đọc được: bản toàn văn Kohavi et al. (sách), ISO/IEC 42001, nội dung giảm thiểu của OWASP ASI, văn bản gốc DSA Điều 25.
8. Cách nói "agent đã vận hành một công ty thật end-to-end" chưa được xác nhận hay bác bỏ bằng nguồn độc lập.

## Nguồn chính

- Uỷ quyền và duyệt: `arxiv.org/abs/2602.11865`, `2609.14767`, `2603.16244`, `2608.18167`, `2605.22534`, `2603.28990`, `2308.00352`, `2411.04468`, `2307.07924`, `2308.08155`
- Phối hợp và trạng thái: `arxiv.org/abs/2512.08296`, `2510.01285`, `2604.03551`, `2607.04697`, `2510.18893`, `2608.23740`, `2606.15376`; `a2a-protocol.org/latest/specification/`; `modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks`; `anthropic.com/engineering/effective-context-engineering-for-ai-agents`; `anthropic.com/engineering/effective-harnesses-for-long-running-agents`; `cognition.com/blog/multi-agents-working`
- Governance: `arxiv.org/abs/2506.12469`; `ai.meta.com/blog/practical-ai-agent-security/`; `artificialintelligenceact.eu/article/14/`; `airc.nist.gov/airmf-resources/playbook/manage/`; `genai.owasp.org`; `modelcontextprotocol.io/specification/2025-11-25/basic/security_best_practices`; `theregister.com` (Replit, Antigravity); `zenity.io/blog/ai-agent-database-deletion-pocketos`; `aws.amazon.com/security/security-bulletins/AWS-2025-015/`
- Phản hồi và KPI: `nber.org/papers/w26463`, `arxiv.org/abs/1603.09326`, `2311.11922`, `2608.08043`, `2309.07893`; `exp-platform.com/Documents/puzzlingOutcomesInControlledExperiments.pdf`; Hohnhold et al. KDD 2015; `arxiv.org/abs/2505.11821`, `2507.08794`, `2603.12246`, `2509.17995`, `1411.2664`, `1502.04585`; `github.blog/engineering/infrastructure/scientist/`
- Năng lực và kinh tế: `arxiv.org/abs/2412.14161`, `2510.04374`, `2510.26787`, `2503.14499`, `2607.01904`; `anthropic.com/research/project-vend-2`; `metr.org/blog/2026-01-29-time-horizon-1-1/`; `epoch.ai/data-insights/llm-inference-price-trends`; `a16z.com/llmflation-llm-inference-cost/`; `tobyord.com/writing/hourly-costs-for-ai-agents`; `metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/`; `evals.alignment.org/blog/2026-02-24-uplift-update/`
