# Phản biện 06: prior art và hype

> Người phản biện prior art và hype của hội đồng. Ngày 2026-11-01. Chỉ đọc và tra cứu; không sửa bản nháp nào.
> Đọc: `01-strategy.md`, `02-design.md`, `03-ledger.md`, `04-launch.md` (toàn văn), `research-ai-autonomous-system-design.md`
> (mục prior art), `papers/INDEX.md`. Trích văn bản PDF trong kho (pypdf, venv tạm `/tmp/nv06`, đã xoá) cho 2602.11865,
> 2603.28990, 2506.12469. Web: 35 lượt tìm và 6 lượt mở trang (4 thành công), liệt kê ở mục 8.
> Nhãn: **HIGH** = tôi đọc nguồn gốc (trang hoặc PDF) trong lần này; **MEDIUM** = chỉ đọc kết quả tìm kiếm, tóm tắt thứ cấp hoặc
> trang vendor; **LOW** = suy luận. Hai trang không mở được (McKinsey, hai lần quá 60 giây): ghi MEDIUM theo kết quả tìm kiếm.

## 0. Kết luận ngắn

- **Không thành phần nào của blueprint là phát minh.** Bảy trên tám thành phần đã được công bố hoặc là tổ hợp của các ý đã công bố
  (mục 2). Thành phần lệch khỏi mọi tài liệu tôi tìm thấy là **gate chỉ từ chối vì hai lý do (dừng và luật)**. Nó lạ vì mọi khung
  quản trị agent đã công bố đều khuyên điều ngược lại (giới hạn quyền, giới hạn phạm vi hành động), chứ không vì ai đó chưa nghĩ tới.
  Cái lạ đó là bằng chứng *chống*, không phải bằng chứng *cho*.
- **Luận điểm trung tâm ("mua năng lực sinh, tự xây năng lực kiểm chứng") đã được nêu công khai**: Jason Wei, "verifier's law"
  (2025); Karpathy, "generation-verification loop" (06/2025); DeepMind, "contract-first decomposition" (2602.11865, có trong kho mà
  bản nháp không trích). Bản nháp nên trình bày nó như một ý đã có và mình áp dụng, không như một luận điểm của riêng mình.
- **Bản nháp ít hype hơn tôi dự đoán.** `01-strategy.md:85-89` đã tự nói "không thành phần nào ở cột cuối là mới nếu xét riêng".
  Không bản nào dùng "nhân loại", "đầu tiên", "lớn nhất". Hype còn lại nằm ở 16 câu cụ thể (mục 3), phần lớn là câu tuyệt đối
  ("duy nhất", "chưa ai", "tiêu chuẩn chung", "biết chắc") và ba câu tự mâu thuẫn với chính thiết kế.
- **"Một bước cho nhân loại"**: không có bằng chứng ở bất kỳ mức nào để nói điều đó, và quyết định không thí nghiệm giới hạn chặt
  điều blueprint *có thể* chứng minh. Mức cao nhất mà bằng chứng cho phép nói ở mục 4.

## 1. Bản đồ prior art 2024–2026

### 1.1 Học thuật

| Công trình | Nó đề xuất gì | Chỗ chồng với blueprint | Nhãn |
|---|---|---|---|
| Tomašev, Franklin, Osindero, *Intelligent AI Delegation*, arXiv 2602.11865 (02/2026) | Khung uỷ quyền cho agent: "contract-first decomposition", tức chỉ giao việc khi kết quả "has precise verification", nếu không thì phân rã tiếp; sổ "performance-based immutable ledger"; sự kiện giám sát (`TASK_STARTED`, `CHECKPOINT_REACHED`...); panel phân xử; đề xuất mở rộng A2A Task bằng `verification_policy` | Gần như trùng với giao thức objective của `02-design.md:104-115` (mỗi key result có `verification_method`), với ledger chỉ thêm (I4) và với ý lỗi tương quan ("insufficient diversity in delegation targets increases the correlation of failures"). **Khác biệt:** DeepMind bắt phân rã đến khi kiểm được, blueprint cho phép key result `unverifiable` và chỉ ghi lại | HIGH (đọc PDF) |
| Zheng et al., *Agents meet OKR*, arXiv 2311.16542 (2023) | Agent sinh Objective và Key Result theo tầng, giao sub-objective cho agent mới, đánh giá nhiều tầng | Phân rã một objective thành OKR theo role là ý đã công bố từ 2023; đánh giá ở đó là LLM tự đánh giá, không phải checker xác định | MEDIUM |
| Wang et al., *OrgAgent: Organize Your Multi-Agent System like a Company*, arXiv 2604.01020 (04/2026) | Ba tầng governance, execution, compliance ("final verification and output control"); báo cáo +102,73% và −74,52% token trên SQuAD 2.0 | Cấu trúc công ty cho agent, có tầng kiểm cuối | HIGH (đọc abstract) |
| Dochkina, arXiv 2603.28990 (trong kho) | Ngoài kết quả thực nghiệm, đề xuất khung "three-ring constitutional" cho tổ chức agent tự trị; tác giả ghi đây là mô hình lý thuyết, Ring 1 do người giữ | Owner giữ mục tiêu và nút dừng, agent tự tổ chức phần còn lại | HIGH (đọc PDF) |
| Feng, McDonald, Zhang, *Levels of Autonomy for AI Agents*, arXiv 2506.12469 (Knight First Amendment Institute) | Năm mức theo vai người: operator, collaborator, consultant, approver, observer; "autonomy certificate" | Owner của blueprint ở mức observer (mức 5); bài này vẫn giữ công tắc tắt ở mức đó | HIGH (đọc PDF) |
| Chan et al., *Visibility into AI Agents*, arXiv 2401.13138 (FAccT 2024) | Ba biện pháp: agent identifier, real-time monitoring, activity log | Trùng với I3, I4, I12 (mọi hành động hiện ra, log do nền tảng ghi) | MEDIUM |
| Chan et al., *Infrastructure for AI Agents*, arXiv 2501.10114 (2025) | Hạ tầng theo ba chức năng attribution, interaction, response; có identity binding và rollback | Trùng với phục hồi (I4, I15) và ghi ai đã làm | MEDIUM |
| Shavit et al. (OpenAI), *Practices for Governing Agentic AI Systems* (12/2023), `cdn.openai.com/papers/practices-for-governing-agentic-ai-systems.pdf` | Bảy thực hành: đánh giá mức phù hợp, giới hạn không gian hành động, hành vi mặc định, legibility, giám sát tự động, attributability, interruptibility | Blueprint giữ legibility, giám sát tự động, attributability, interruptibility; **bỏ** "constraining the action space" | MEDIUM |
| Kolt, *Governing AI Agents*, arXiv 2501.07913 (Notre Dame L. Rev. 101) | Lý thuyết principal–agent và luật agency áp vào AI agent; incentive, monitoring, enforcement có thể không đủ | Nền lý thuyết cho "owner hẹp, đọc tín hiệu" | MEDIUM |
| Hammond et al., *Multi-Agent Risks from Advanced AI*, arXiv 2502.14143 (2025) | Ba chế độ hỏng: miscoordination, conflict, collusion; bảy yếu tố rủi ro | Phần multi-agent của sổ rủi ro không trích nó | MEDIUM |
| Tomasev et al., *Virtual Agent Economies*, arXiv 2509.10147 (2025) | "Sandbox economy", phân theo nguồn gốc và độ thấm của ranh giới; "mission economies" | Ranh giới doanh nghiệp (`01-strategy.md:188`) là một dạng ranh giới thấm | MEDIUM |
| Perez, *The Law of Stop*, arXiv 2609.22882 (09/2026) | Dừng là bài toán thể chế; khoảng 1.400 sự cố AI, khoảng 80% không có stop được ghi; trong hệ agentic "a stop at one point may leave the activity running elsewhere" | Trực tiếp về F1 và đúng chỗ yếu của nó (L5, `02-design.md:296-299`) | HIGH (đọc abstract) |
| *Agentic Business Process Management: A Research Manifesto*, arXiv 2603.18916 (2026) | "Framed autonomy": tự chủ bị ràng buộc qua nhận thức quy trình | Đối lập với "không giới hạn sáng kiến" | MEDIUM |
| Chủ đề pháp lý cũ hơn: Bayern (2014) về LLC do thuật toán điều khiển; LoPucki, *Algorithmic Entities*, Wash. U. L. Rev. 95(4) (2018) | Thuật toán có thể nắm quyền điều khiển hợp pháp một LLC; rủi ro của thực thể không có người điều khiển | "Công ty do agent vận hành" là câu hỏi pháp lý đã có từ 2014; blueprint giữ một owner là thể nhân (`02-design.md:426`), đúng với lo ngại này | MEDIUM |

Ghi chú: phần lớn bằng chứng thực nghiệm mà bản nháp dùng (MetaGPT, Magentic-One, Kim 2512.08296, Loop-Back, judge tương quan) đã có
trong kho và được trích đúng. Thiếu là phần **khung quản trị và tổ chức** ở trên.

### 1.2 Tiêu chuẩn và khung quản trị

| Khung | Đề xuất gì | Chồng hoặc đối lập | Nhãn |
|---|---|---|---|
| Singapore IMDA, *Model AI Governance Framework for Agentic AI* (22/01/2026, Davos), `mddi.gov.sg` | Bốn trục: (1) "bounding the risks upfront ... placing limits on agents' powers such as agents' autonomy and access to tools and data"; (2) "defining significant checkpoints at which human approval is required"; (3) kiểm soát kỹ thuật, kể cả "whitelisted services"; (4) minh bạch cho người dùng cuối | Khung quốc gia đầu tiên riêng cho agent (theo CMS, Baker McKenzie). Blueprint **đối lập** ở trục 1, 2 và một phần trục 3 | HIGH (đọc trang MDDI) |
| NIST AI RMF; NIST NCCoE concept paper *Accelerating the Adoption of Software and AI Agent Identity and Authorization* (05/02/2026) | Định danh, uỷ quyền, kiểm toán, non-repudiation; chủ đề ưu tiên "least-privilege authorization by design", "prompt injection as a control-design problem" | Trùng ở định danh và kiểm toán; đối lập ở least privilege | MEDIUM |
| ISO/IEC 42001:2023 | Hệ thống quản lý AI | `04-launch.md:233` đề xuất ánh xạ vào; chưa ai trong hội đồng đọc văn bản (`design.md:273`) | MEDIUM |
| OWASP Top 10 for Agentic Applications (09/12/2025) | ASI01 Agent Goal Hijack ... ASI10 Rogue Agents | Đã trích trong `research-…-design.md`; nhiều biện pháp là giới hạn mà blueprint bỏ | MEDIUM |
| CSA MAESTRO (02/2025, Ken Huang) | Threat modeling bảy lớp cho agentic AI | Phương pháp, không phải mô hình vận hành; bản nháp chưa dùng | MEDIUM |
| AIUC-1 (2025, Artificial Intelligence Underwriting Company) | Chuẩn chứng nhận cho agent, sáu trụ (data và privacy, security, safety, reliability, accountability, society), audit bên thứ ba, gắn với bảo hiểm | Đối thủ trực tiếp của ý "scorecard thành tiêu chuẩn chung" (`04-launch.md:191`) | MEDIUM |
| Google, *An Introduction to Google's Approach for Secure AI Agents* (Díaz, Kern, Olive, 05/2025) | Ba nguyên tắc: agent có người điều khiển rõ ràng, "powers must be carefully limited", hành động và kế hoạch phải quan sát được; defense in depth trộn kiểm soát xác định với phòng thủ dựa trên suy luận | Trùng nguyên tắc 1 và 3; đối lập nguyên tắc 2 | MEDIUM |
| Microsoft AI Red Team, *Taxonomy of Failure Modes in Agentic AI Systems* (04/2025; cập nhật 06/2026 thêm 7 loại) | Phân loại chế độ hỏng mới và cũ của agent | Sổ rủi ro `03-ledger.md` §2 nên đối chiếu | MEDIUM |

### 1.3 Hãng tư vấn và vendor

| Bên | Đề xuất gì | Chồng ở đâu | Nhãn |
|---|---|---|---|
| McKinsey, *The agentic organization: Contours of the next paradigm for the AI era* (2025) | Gọi đây là thay đổi mô hình tổ chức lớn nhất kể từ cách mạng công nghiệp và số; năm trụ (business model, operating model, technology và data, talent và organization, governance); workflow "AI-first"; đội nhỏ hướng kết quả | Cùng tham vọng "công ty vận hành bằng agent". Không thấy số liệu lợi nhuận được kiểm chứng trong phần tôi đọc được | MEDIUM (không mở được trang) |
| Microsoft, *2025 Work Trend Index: The Frontier Firm is born* (23/04/2025) | "Frontier Firm", "agent boss", chỉ số "human-agent ratio" | Owner chỉ định hướng tương ứng với "agent boss"; đây là khảo sát và tầm nhìn, không phải thiết kế | MEDIUM |
| Microsoft Agent 365 (Ignite, 11/2025) | "Control plane" cho agent: registry, access control, visualization, interop, security; mỗi agent có Entra Agent ID | Blueprint dùng đúng từ "control plane" (`02-design.md:75-84`) | MEDIUM |
| ServiceNow AI Control Tower (Knowledge 2025; mở rộng 2026: Discover, Observe, Govern, Secure, Measure; "AI Passport" ký số kiểm ở runtime) | Một chỗ trung tâm quản trị mọi agent, model, workflow | Trùng với control plane cộng bề mặt quan sát | MEDIUM |
| Salesforce, Agentforce 360 và "Agentic Enterprise" (14/10/2025) | Người, agent và dữ liệu trên một nền tảng; "hybrid reasoning" trộn workflow quy tắc với LLM | Trộn quy tắc xác định với LLM là cùng hướng với tầng 0 checker | MEDIUM |
| Intercom Fin: giá $0,99 mỗi "outcome" | Đơn vị thương mại là một lần giải quyết không cần chuyển cho người | Tiền lệ cho "kết quả đã xác minh" làm đơn vị (`04-launch.md:136`); định nghĩa của vendor không có cửa sổ 30 ngày | MEDIUM |
| Credential broker: Auth0 Token Vault; Arcade.dev ("LLMs never see OAuth tokens") | Agent gọi API mà không giữ token; token chỉ chèn lúc thực thi | **F2 là sản phẩm thương mại đã có**, không phải điểm khác biệt | MEDIUM |

### 1.4 Mã nguồn mở

| Dự án | Đề xuất gì | Chồng ở đâu | Nhãn |
|---|---|---|---|
| Paperclip (`github.com/paperclipai/paperclip`, ra 02/03/2026), tự mô tả "open-source orchestration for zero-human companies" | Org chart cho agent; task gắn project, goal và mission công ty; ngân sách nhiều tầng có hard stop; "board approval workflows"; cấu hình có phiên bản, rollback; activity log bền; atomic checkout chống hai agent nhận cùng việc; "secret injection" | Đây là **prior art gần nhất** cho cả blueprint: mục tiêu công ty xuống task (≈ objective → OKR), claim (≈ atomic checkout), sổ release (≈ config versioning), log. Khác: Paperclip có trần chi phí và cổng duyệt, blueprint bỏ cả hai. Không thấy Paperclip có checker xác định cho số khách, đo độ độc lập judge, hay scorecard công khai | HIGH cho README (đã mở); số sao (71.000) là MEDIUM |
| agentgateway (Linux Foundation) | Proxy cho LLM, MCP, A2A, có RBAC, quan sát, governance | Hiện thực mở của "một gate" (I1) | MEDIUM |
| LangGraph | State có schema (TypedDict hoặc Pydantic), mỗi key có reducer riêng | Ngữ cảnh chung có kiểu, ghi theo mục (I13) | MEDIUM (đoạn trích tài liệu chính thức qua tìm kiếm) |
| HAL, *Holistic Agent Leaderboard*, arXiv 2510.11977 (ICLR 2026) | Leaderboard bên thứ ba có chi phí; 21.730 rollout; công bố toàn bộ log 2,5 tỷ token | Tiền lệ cho công bố log và đo chi phí cùng chất lượng | MEDIUM |
| MIT AI Agent Index (Casper et al., arXiv 2502.01635) | Cơ sở dữ liệu công khai về agent đã triển khai; nhận xét vendor công bố ít về an toàn | Tiền lệ cho "mở" ở cấp ngành | MEDIUM |
| MetaGPT, ChatDev, AutoGen, CrewAI, Magentic-One | Đã trích trong `01-strategy.md:74-83` | — | — |

## 2. Phân loại từng thành phần

| # | Thành phần | Phân loại | Nguồn đã có | Phần thật sự là của blueprint, và vì sao có thể vẫn sai |
|---|---|---|---|---|
| E1 | Một objective → OKR theo role, có duyệt kiểm chứng | **TỔ HỢP** | OKR-Agent 2311.16542; contract-first 2602.11865; Paperclip (goal → task, board approval); MetaGPT 2308.00352; Contract Net (Smith 1980) | Duyệt **không chặn**, một vòng trả lời, `reason_code` đóng. Sai nếu: tỷ lệ key result `unverifiable` cao và kéo dài, vì DeepMind lập luận rằng việc không kiểm được phải phân rã tiếp chứ không được chạy; blueprint chọn ngược lại mà không có số đo nào |
| E2 | Ngữ cảnh chung có kiểu | **TỔ HỢP** | Blackboard (Hearsay-II, Erman et al., ACM Computing Surveys 1980); 2510.01285 (trong kho); LangGraph state và reducer; MetaGPT message pool; ledger của Magentic-One; ACE 2510.04618 (cập nhật tăng dần); `base_version` là optimistic concurrency kiểu ETag và If-Match của HTTP; taint như CaMeL 2503.18813 và FIDES 2505.23643 | Taint **chỉ ghi, không chặn**, tức yếu hơn có chủ ý so với CaMeL và FIDES. Sai nếu: phiên `untrusted` gây hành động có hại mà tripwire chỉ báo sau khi đã xảy ra (W8) |
| E3 | Một gate, hai lý do từ chối | **Gate: ĐÃ CÔNG BỐ. Hai lý do: MỚI theo nghĩa đi ngược mọi khung** | Reference monitor (Anderson, 1972); complete mediation (Saltzer và Schroeder, Proc. IEEE 1975); agentgateway; Agent 365; ServiceNow; credential broker Auth0 và Arcade | Không khung nào tôi tìm thấy khuyên chỉ từ chối vì dừng và luật: Singapore MGF, Google, OpenAI (Shavit), NIST NCCoE, OWASP đều khuyên giới hạn quyền. Sai nếu: sự cố đầu tiên đi qua một connection hợp lệ (confused deputy, `02-design.md:301-303`) gây tổn thất không đảo ngược được mà một giới hạn phạm vi sẽ chặn; hoặc luật sư trả lời câu 1 ở `03-ledger.md:371-373` (QĐ 33/2026) là phải có người kiểm trước, khi đó F3 tự buộc một lý do từ chối thứ ba |
| E4 | Checker xác định cho số khách dựa vào | **ĐÃ CÔNG BỐ** | PAL (Gao et al., arXiv 2211.10435); Program of Thoughts (Chen et al., arXiv 2211.12588); GitHub Scientist; đối soát và tách nhiệm vụ trong kiểm soát nội bộ tài chính; kết luận của FinSheet-Bench 2603.07316 | Không có phần mới. I10 (truy mọi con số về một hàm có phiên bản) là data lineage. Đây là quyết định đúng và nên nói là thực hành chuẩn, không phải đóng góp |
| E5 | Đo độ độc lập của judge | **TỔ HỢP**, có một quy tắc vận hành nhỏ có thể là mới | Verga et al., PoLL, arXiv 2404.18796 (panel nhiều họ model); Kohli 2605.29800 và Hossain 2609.22512 (n_eff); Kuai 2604.07650 (gộp có tính phụ thuộc) | Quy tắc I9 "panel không có ρ và n_eff trên tập neo thì tính là một phiếu" tôi không tìm thấy ở đâu dưới dạng quy tắc vận hành (**MEDIUM**: không tìm thấy không có nghĩa là không có). Sai nếu: tập neo nhỏ hoặc cũ làm ρ đo trên neo khác ρ trên dữ liệu thật (U3 ở `03-ledger.md:401`) |
| E6 | Hàng đợi ngoại lệ của owner | **ĐÃ CÔNG BỐ** | Kale 2508.19461 (chỉ ca được gắn cờ tới người); "automatic monitoring" của Shavit; quản trị theo ngoại lệ là khái niệm quản trị lâu đời (**LOW** về nguồn gốc, chưa tra); dashboard của ServiceNow, Agent 365 | Phần riêng là: owner *không* là lưới chặn, và lỗi gieo không vào hàng đợi của owner. Sai nếu: W9 cho thấy lỗi tìm thấy sau merge không giảm, tức hàng đợi chỉ chuyển vai "người duyệt mọi thứ" sang "người duyệt ngoại lệ" mà không cải thiện |
| E7 | Scorecard công khai | **TỔ HỢP** | pass^k (τ-bench 2406.12045); độ tin cậy (Rabanser 2602.16666); strict pass (CXBench 2610.25954); DORA four keys (Forsgren, Humble, Kim, *Accelerate*, 2018); HAL công bố log; AIUC-1; đơn vị "outcome" của Intercom | Công bố scorecard của **vận hành thật** theo từng release, kể cả kết quả xấu, với đơn vị "kết quả đã xác minh" có cửa sổ 30 ngày: tôi không tìm thấy công ty nào làm vậy (**MEDIUM**). Sai nếu: không bên thứ hai nào áp dụng định nghĩa; khi đó nó là báo cáo của một công ty, không phải thước đo chung |
| E8 | Triển khai một lần | **ĐÃ CÔNG BỐ, và được ghi nhận là mẫu thất bại** | "Big bang" trong triển khai ERP: Hershey 1999 cắt chuyển cùng lúc SAP R/3, Siebel, Manugistics, không giao được khoảng 100 triệu USD đơn hàng Halloween (CIO.com, **MEDIUM**); SRE Workbook khuyên canary; Anthropic cam kết "gradual rollouts" (`04-launch.md:294`) | Không mới. Sai nếu: trong 12 tuần đầu không quy được nguyên nhân cho đa số sự cố (R0 ở `03-ledger.md:176-184` đã dự báo điều này) |

**Tổng (MEDIUM):** 0 thành phần là phát minh; 3 đã công bố (E4, E6, E8); 4 là tổ hợp (E1, E2, E5, E7); 1 có một phần đi ngược
mọi khung (E3, hai lý do từ chối). Điều có thể gọi là riêng của blueprint là **tập hợp các lựa chọn đứng cùng nhau**: không giới
hạn sáng kiến, đi cùng kiểm chứng xác định, đo độ độc lập judge, lỗi gieo, và công bố vận hành thật. Tổ hợp đó tôi không tìm thấy
ở Paperclip, McKinsey, Microsoft, ServiceNow hay trong bài học thuật nào. Nó cũng chưa được chứng minh có ích.

## 3. Câu khẳng định tính mới hoặc tầm quan trọng trong bản nháp

Tìm bằng `rg` các từ "mới", "đầu tiên", "duy nhất", "chưa ai", "không ai", "lớn nhất", "mọi công ty", "tiêu chuẩn", "chuẩn chung",
"thế giới", "biết chắc", "đầy đủ", "chứng minh", "nhân loại" và tương đương tiếng Anh trên bốn bản nháp, rồi đọc từng kết quả.
Câu dùng "duy nhất" theo nghĩa thiết kế (ví dụ `02-design.md:83`, `04-launch.md:163`) không phải claim tính mới nên không liệt kê.
**Không bản nào chứa "nhân loại", "đầu tiên" (như claim), "lớn nhất" (về mình) hay "mọi công ty".**

| # | file:line | Câu (rút gọn) | Phán quyết | Thay bằng |
|---|---|---|---|---|
| 1 | `01-strategy.md:37-40` | Thứ khan hiếm là năng lực kiểm chứng; "mua năng lực sinh, tự xây năng lực kiểm chứng" | Ý đã công bố, trình bày như luận điểm riêng | "Luận điểm này áp vào cấp công ty một ý đã được nêu: verifier's law (Wei 2025), generation-verification loop (Karpathy 2025), contract-first decomposition (2602.11865). Phần của bài là cách áp nó và cách đo (giả thuyết)." |
| 2 | `01-strategy.md:58` | Tiêu đề "cái gì thật sự mới" | Hứa nhiều hơn nội dung (nội dung ở dòng 85 nói là không mới) | "Đã có gì, và bài này ghép lại thế nào" |
| 3 | `01-strategy.md:79` | "không framework nào có bước duyệt objective kèm lý do bằng văn bản" | Quá tuyệt đối; Paperclip có board approval, OKR-Agent có đánh giá nhiều tầng | "Trong CrewAI, OpenAI Agents SDK, ADK, LangGraph không thấy bước duyệt objective có mã lý do đóng; Paperclip có duyệt của board nhưng không có mã lý do (MEDIUM)." |
| 4 | `01-strategy.md:79` (cột cuối) và `:213` | Framework thiếu "niêm credential"; bài này thêm niêm credential | Đã là sản phẩm: Auth0 Token Vault, Arcade, secret injection của Paperclip | "Niêm credential là thực hành có sẵn (Auth0, Arcade); blueprint dùng nó làm điểm cố định." |
| 5 | `01-strategy.md:80` | "Agent tự xây kiểm chứng cho chính chỗ Devin nêu là yếu" | Không có bằng chứng; verifier's law dự đoán đúng chỗ đó là khó nhất | "Giả thuyết: agent xây được kiểm chứng ở một phần việc mà Devin nêu là khó kiểm. Chưa có số đo." |
| 6 | `01-strategy.md:81` | "Việc không dừng ở PR; owner không là lưới chặn cuối" | Mâu thuẫn với F4: merge do owner (`02-design.md:197`, `04-launch.md:36-39`) | Sửa sau khi hội đồng chốt rail merge; đến lúc đó: "Việc theo dõi tiếp sau merge (ADR-0013); owner vẫn kéo merge." |
| 7 | `01-strategy.md:87-88` | "Cái mới là giao toàn bộ công ty cho agent cùng một lúc..." | Tham vọng không mới (Paperclip "zero-human companies", McKinsey, Polsia và NanoCorp, Project Vend); "cùng một lúc" là big bang | "Điểm riêng là tổ hợp: không giới hạn sáng kiến, đi cùng kiểm chứng xác định, đo độ độc lập judge, và công bố vận hành thật. Chưa ai, kể cả chúng tôi, cho thấy tổ hợp này có lãi." |
| 8 | `01-strategy.md:165` | Bật một lần "tăng tốc" việc học | Chỉ tăng lượng dữ liệu; theo R0 (`03-ledger.md:181-184`) nó làm giảm khả năng quy nguyên nhân | Thêm: "và làm khó quy nguyên nhân hơn (R0); lợi ròng cho việc học chưa biết dấu. LOW." |
| 9 | `01-strategy.md:218` | "hệ thống vận hành để biết chắc" | Tự mâu thuẫn: `02-design.md:181` nói tác động nhân quả không xác định được | "Hệ thống vận hành để kiểm chứng từng kết quả và đo xem bộ kiểm chứng đáng tin đến đâu." |
| 10 | `01-strategy.md:219` | "người duyệt mọi thứ", "đã được đo là thất bại" | Suy rộng: 94% là trong bài đối kháng 5 giờ; Vaccaro cho thấy người + AI hơn cả hai khi người giỏi hơn | "Đã được đo là kém trong các điều kiện đối kháng và khi khối lượng duyệt lớn [R §2.1]." |
| 11 | `01-strategy.md:286` | "tạo ra dữ liệu mà chưa ai có" | Có điều kiện; HAL đã công bố 2,5 tỷ token log, nhưng là benchmark | "Nếu công bố theo phương pháp ở `04-launch.md` §4, đây là một chuỗi dữ liệu thực địa hiếm (n = 1 công ty) về vận hành không giới hạn." |
| 12 | `02-design.md:210-211` | "Thiết kế đo được duy nhất là tách luồng điều khiển khỏi dữ liệu" (CaMeL) | Sai: FIDES 2505.23643 cũng đo trên AgentDojo, chặn mọi injection khi bật policy check | "Các thiết kế có số đo trên AgentDojo gồm CaMeL (2503.18813) và FIDES (2505.23643), cùng dựa vào kiểm soát luồng thông tin." |
| 13 | `02-design.md:244-246` | "Hệ thống tối thiểu đầy đủ": không thiếu gì để tổn thất ngày 0 có cận | Mâu thuẫn với điểm yếu 1 ở `02-design.md:432-433` (hành động không đảo ngược thì không có cận) | "Hệ thống tối thiểu để phát hiện, dừng và phục hồi. Tổn thất của hành động ra ngoài không đảo ngược được vẫn không có cận." |
| 14 | `02-design.md:348-349` | Property test của `Resolve` "chứng minh không có giới hạn ẩn" | Chỉ chứng minh cho một hàm thuần; giới hạn ẩn có thể nằm ở scope OAuth, rate limit của provider, sandbox | "...chứng minh `Resolve` không chứa giới hạn ẩn; các đường khác (scope của connection, sandbox) cần diễn tập I1." |
| 15 | `04-launch.md:133-134`, `:149`, `:190-191` | Scorecard cho "các công ty khác một thước đo chung", "biến scorecard thành tiêu chuẩn chung" | Một chuẩn cần bên thứ hai áp dụng và một nơi giữ định nghĩa; đã có AIUC-1, HAL, DORA | "Định nghĩa mở mà công ty khác có thể dùng. Nó chỉ thành thước đo chung khi ít nhất một tổ chức độc lập áp dụng và tái lập." |
| 16 | `04-launch.md:245` | "để thế giới có dữ liệu về vùng chưa ai đo" | Tuyệt đối; Pan đo 86 hệ thống, không đo mọi hệ thống | "...về vùng mà khảo sát của Pan (86 hệ thống, 68% ≤ 10 bước) không phủ." |
| — | `04-launch.md:47` | "Một thành phần thiếu ở ngày 0 không bao giờ có dữ liệu nền" | Tuyệt đối, nhỏ: thiếu mốc trước thay đổi, không phải thiếu mọi dữ liệu nền | "...không có mốc trước ngày 0 để so." |

`03-ledger.md` không có claim tính mới. Mục 5 của nó ("Khẳng định chúng tôi không đưa ra") là mẫu tốt nhất trong bốn bản.

## 4. "Bước tiến cho nhân loại": câu trả lời trung thực

**Bằng chứng hiện có không cho phép nói điều đó, ở bất kỳ mức nhãn nào.** Lý do:

1. Chưa có ca A hoặc B nào cho thấy công ty vận hành đầu cuối bằng agent có lợi nhuận được xác minh (`03-ledger.md:411`, U13);
   trong CEO-Bench mọi model thua baseline theo quy tắc (2606.18543). **HIGH** trong phạm vi đã tìm.
2. Ước lượng vĩ mô còn khiêm tốn: Acemoglu, *The Simple Macroeconomics of AI*, NBER w32487 (2024): tối đa +0,66% TFP trong 10 năm,
   có thể dưới 0,53%; lãnh đạo tự báo năng suất +0,2%/năm (NBER w34836). **MEDIUM** cho Acemoglu (đọc tóm tắt).
3. Blueprint không thêm năng lực model, thuật toán hay giao thức (`01-strategy.md:88-89` tự nói). Một thay đổi cỡ "nhân loại" đến
   từ blueprint vận hành chỉ có thể đến qua **áp dụng rộng và kết quả đo được**, và cả hai đều chưa tồn tại.
4. Quyết định không thí nghiệm giới hạn điều có thể chứng minh: không có đối chứng thì kết quả là tương quan theo thời gian
   (`01-strategy.md:152-155`). Tôi lưu ý một sắc thái mà bản nháp bỏ qua: các phương pháp **bán thực nghiệm** trên dữ liệu quan sát,
   như synthetic control (Abadie, Diamond, Hainmueller, JASA 2010) hay Bayesian structural time series (Brodersen et al., Annals of
   Applied Statistics 2015, "CausalImpact"), không chạy thí nghiệm trên khách và cho bằng chứng nhân quả yếu hơn nhưng khác 0, *nếu*
   có đơn vị so sánh chưa bật (công ty khác, thị trường khác). Bật toàn bộ cùng lúc bên trong ranh giới loại bỏ đơn vị so sánh bên
   trong; đơn vị bên ngoài vẫn dùng được. `02-design.md:181` ("không xác định được") nên đổi thành "chỉ ước lượng được bằng phương
   pháp bán thực nghiệm với đơn vị so sánh bên ngoài, độ tin thấp hơn thí nghiệm". **MEDIUM.**

**Mức cao nhất bằng chứng cho phép nói hôm nay:** "Một phương pháp vận hành mở, có thể bác bỏ được, cho công ty giao phần lớn việc
cho agent, cùng cam kết công bố dữ liệu vận hành thật, kể cả thất bại."

**Bằng chứng cần có trước khi nói lớn hơn** (mỗi mục là điều kiện cần, không mục nào đủ một mình):

| Claim muốn nói | Bằng chứng tối thiểu |
|---|---|
| "Có lãi" | Báo cáo tài chính được kiểm toán độc lập của đơn vị vận hành bằng agent, ≥ 12 tháng, với chi phí trọn gói (S1 ở `01-strategy.md:267`) và một đơn vị so sánh bên ngoài |
| "Tốt hơn cách hiện có" | So với ít nhất một lựa chọn ở `01-strategy.md:209-216` trên cùng loại việc, cùng định nghĩa "kết quả đã xác minh" |
| "Tổng quát" | Kết quả ở ≥ 3 lớp việc, trong đó ít nhất một lớp không phải code (TheAgentCompany cho thấy tài chính và hành chính yếu nhất) |
| "An toàn" | Sự cố trên mỗi 1.000 kết quả đã xác minh, theo mức nặng, ≥ 12 tháng; diễn tập dừng M7 đạt mục tiêu công bố |
| "Thành chuẩn" | ≥ 2 tổ chức độc lập áp dụng và công bố M1–M7, một bên tính lại số từ bảng mỗi lần chạy (`04-launch.md:188`) và khớp |
| "Bước tiến cho nhân loại" | Tất cả các dòng trên, cộng áp dụng ở quy mô ngành với tác động đo được lên năng suất hoặc giá cho khách. Không có trong tầm của một công ty |

## 5. Bài toán mở lớn nhất mà blueprint đóng góp được

**Bài toán:** biết một kết quả do agent làm ra là đúng, trong vận hành thật, mà không cần người đọc lại, và biết bộ kiểm chứng
đó đáng tin đến đâu. Bằng chứng rằng đây là bài toán mở: 74% hệ thống đang chạy dựa chủ yếu vào đánh giá của người và 68% dừng cho
người sau ≤ 10 bước (Pan 2512.04123, **HIGH** theo `03-ledger.md:141`); độ tin cậy tăng chậm hơn độ chính xác (2602.16666); judge
LLM sai tương quan kể cả khi khác nhà cung cấp (2609.22512, 2605.29800); DeepMind coi "verifiable task completion" là yêu cầu chưa
giải của uỷ quyền (2602.11865 §4.8, **HIGH**); HAL tự gọi mình là "the missing infrastructure for AI agent evaluation".

**Đóng góp nói chính xác (đề xuất câu cho báo cáo cuối):** "Blueprint đóng góp một chuỗi dữ liệu thực địa mở, n = 1 tổ chức, về ba
đại lượng mà tài liệu hiện chỉ đo trên benchmark: (a) tỷ lệ bắt lỗi của checker và auditor, đo bằng lỗi gieo từ lịch sử (W5);
(b) độ độc lập thực của panel judge trên dữ liệu kinh doanh thật, ρ và n_eff trên tập neo (W6, U3); (c) tỷ lệ lỗi thoát trong 30
ngày theo lớp việc (M3), ở cấu hình tự chủ không giới hạn mà khảo sát của Pan không phủ." Đóng góp này **không** gồm bằng chứng
nhân quả về lợi nhuận. **MEDIUM**: giá trị của nó phụ thuộc vào việc công bố thật và đủ lâu.

## 6. Prior work bản nháp nên trích (đã xác minh tồn tại)

Ưu tiên theo mức liên quan. "Đã đọc" = tôi mở nguồn gốc trong lần này.

1. Tomašev, Franklin, Osindero. *Intelligent AI Delegation*. arXiv:2602.11865 (2026). **Đã có trong kho**, chỉ báo cáo design trích;
   giao thức objective của `02-design.md` §2.3 phải trích nó. Đã đọc.
2. Paperclip. `github.com/paperclipai/paperclip` (2026). Prior art gần nhất cho cả hệ thống; đặt vào bảng `01-strategy.md:74-83`. Đã đọc README.
3. IMDA Singapore. *Model AI Governance Framework for Agentic AI* (22/01/2026). `mddi.gov.sg/newsroom/singapore-launches-new-model-ai-governance-framework-for-agentic-ai--/`. Đã đọc.
4. Perez, O. *The Law of Stop: Interruptibility, Injunctions, and the Governance of Agentic AI*. arXiv:2609.22882 (2026). Cho F1. Đã đọc abstract.
5. Shavit et al. *Practices for Governing Agentic AI Systems*. OpenAI (2023). `cdn.openai.com/papers/practices-for-governing-agentic-ai-systems.pdf`.
6. Zheng, Ma, Shi, Huang. *Agents meet OKR*. arXiv:2311.16542 (2023).
7. Wang et al. *OrgAgent: Organize Your Multi-Agent System like a Company*. arXiv:2604.01020 (2026). Đã đọc abstract.
8. Chan et al. *Visibility into AI Agents*. arXiv:2401.13138 (FAccT 2024); Chan et al. *Infrastructure for AI Agents*. arXiv:2501.10114 (2025).
9. Wei, J. *Asymmetry of verification and verifier's law*. `jasonwei.net/blog/asymmetry-of-verification-and-verifiers-law` (2025);
   Karpathy, keynote YC AI Startup School (06/2025), ghi chép tại `latent.space/p/s3`. Cho luận điểm ở `01-strategy.md:37-41`.
10. Costa et al. (Microsoft). *Securing AI Agents with Information-Flow Control* (FIDES). arXiv:2505.23643 (2025). Cho `02-design.md:210`.
11. Verga et al. *Replacing Judges with Juries* (PoLL). arXiv:2404.18796 (2024). Tiền lệ của panel nhiều họ model (I9).
12. NIST NCCoE. *Accelerating the Adoption of Software and AI Agent Identity and Authorization*, concept paper (05/02/2026).
13. Díaz, Kern, Olive. *An Introduction to Google's Approach for Secure AI Agents* (05/2025), `research.google/pubs/an-introduction-to-googles-approach-for-secure-ai-agents/`.
14. Kolt, N. *Governing AI Agents*. arXiv:2501.07913, Notre Dame L. Rev. 101. Hammond et al. *Multi-Agent Risks from Advanced AI*. arXiv:2502.14143.
15. Kapoor et al. *Holistic Agent Leaderboard*. arXiv:2510.11977 (ICLR 2026); Casper et al. *The AI Agent Index*. arXiv:2502.01635. Cho `04-launch.md` §4.
16. AIUC-1 (2025); CSA MAESTRO (`cloudsecurityalliance.org/blog/2025/02/06/agentic-ai-threat-modeling-framework-maestro`);
    Microsoft AI Red Team, *Taxonomy of Failure Modes in Agentic AI Systems* (04/2025, cập nhật 06/2026).
17. Vendor và tư vấn, cho mục định vị `01-strategy.md` §5: McKinsey *The agentic organization* (2025); Microsoft *2025 Work Trend
    Index: The Frontier Firm is born*; Microsoft Agent 365; ServiceNow AI Control Tower; Salesforce Agentforce 360; Auth0 Token
    Vault và Arcade.dev (cho F2); agentgateway (cho I1).
18. Kinh điển cho "một gate": Saltzer và Schroeder, *The Protection of Information in Computer Systems*, Proc. IEEE 63(9), 1975
    (complete mediation); Anderson, *Computer Security Technology Planning Study*, 1972 (reference monitor). Cho ngữ cảnh chung:
    Erman et al., *The Hearsay-II Speech-Understanding System*, ACM Computing Surveys 1980 (blackboard). Cho checker số:
    Gao et al., PAL, arXiv:2211.10435; Chen et al., Program of Thoughts, arXiv:2211.12588. Ba mục này tôi trích theo hiểu biết,
    không mở lại lần này: **MEDIUM**.
19. Cho "không thí nghiệm": Abadie, Diamond, Hainmueller, JASA 105 (2010), synthetic control; Brodersen et al., Annals of Applied
    Statistics 9(1) (2015), CausalImpact. Acemoglu, NBER w32487 (2024). Hershey 1999 (CIO.com, "Hershey's bittersweet lesson") cho
    rủi ro big bang. **MEDIUM** (không mở toàn văn).

## 7. Điều mạnh

- **Tự đánh giá tính mới đã trung thực** ở `01-strategy.md:85-89`: nói thẳng từng thành phần đến từ đâu (SRE bake time, kiểm soát
  nội bộ tài chính). Đây là câu nên giữ nguyên trong báo cáo cuối.
- **Sổ claim có kiểm lại toàn văn** (`03-ledger.md` §1, 54 hàng RECHECKED, mục 1.10 sửa mẫu số và đơn vị). Hiếm thấy ở báo cáo
  ngành; McKinsey và Microsoft không công bố mức này cho số liệu của họ.
- **Danh sách khẳng định không đưa ra** (`03-ledger.md` §5) và câu "không có nguồn lợi tức nào từ cắt 80% chi phí review"
  (`01-strategy.md:102`): chống hype bằng cấu trúc, không bằng lời hứa.
- **Invariant có phép thử bác bỏ** (`02-design.md` §1) và property test trên hàm thuần (§5): biến nguyên tắc thành thứ kiểm được.
  Các khung quản trị ở mục 1.2 là nguyên tắc; chúng không có phép thử tương ứng.
- **Đo bộ kiểm chứng thay vì tin nó**: lỗi gieo (W5), n_eff (I9), tập neo ghim cùng phiên bản judge. Đây là phần gần nhất với
  một đóng góp thật cho bài toán ở mục 5.
- **Điểm yếu tự nhận** (`02-design.md` §9) và pre-mortem (`04-launch.md` §6) nói rõ chỗ thiết kế không che được (hành động không
  đảo ngược, injection qua công cụ hợp lệ).

## 8. Nguồn đã dùng trong bản này và giới hạn

- **Đã mở trang hoặc PDF:** `mddi.gov.sg` (Singapore MGF); `github.com/paperclipai/paperclip`; `arxiv.org/abs/2609.22882`;
  `arxiv.org/abs/2604.01020`; PDF trong kho 2602.11865, 2603.28990, 2506.12469.
- **Không mở được:** trang McKinsey (hai URL, quá 60 giây). Nội dung lấy từ kết quả tìm kiếm.
- **Chỉ qua kết quả tìm kiếm (MEDIUM):** mọi nguồn còn lại ở mục 1 và 6. Tôi không trích con số nào từ chúng ngoài con số đã có
  trong kết quả tìm kiếm.
- **Giới hạn phạm vi:** 35 lượt tìm, 6 lượt mở trang (4 thành công). Không tìm: Deloitte, BCG, Accenture, Gartner ("guardian agents"), IBM,
  AWS (Bedrock AgentCore), Google Agentspace; ISO/IEC 42005 và 42006; tài liệu tiếng Trung và tiếng Nhật. Vì vậy "không tìm thấy
  tổ hợp này ở ai" (mục 2) là **coverage, không phải chứng minh không tồn tại**.
- Assumption: "blueprint" là bốn bản nháp hội đồng, vì đó là đầu vào được giao; `challenge-03-security.md` có trong thư mục nhưng
  không thuộc phạm vi phản biện này nên tôi không đọc.
- Assumption: "genuinely new" nghĩa là không tìm thấy trong tài liệu công bố 2023–2026 tôi tra được, vì chứng minh không tồn tại là
  bất khả; mỗi phán quyết "mới" ở mục 2 vì vậy kèm một lý do có thể làm nó sai.
