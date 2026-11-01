//! Bai Yan's notes: one page per chapter. The sentence he wrote beside the
//! grid pattern reads like madness until the chapter is done; then ORI's
//! correction says what it meant.

use oo_base::Tr;

pub struct Note {
    /// What Bai Yan wrote.
    pub line: Tr,
    /// ORI's correction, shown once the chapter's commission is done.
    pub meaning: Tr,
}

const fn n(line: Tr, meaning: Tr) -> Note {
    Note { line, meaning }
}

static NOTES: [Note; 16] = [
    n(
        Tr::new("\"Thirty-two of me crossed one river at once.\"", "“三十二个我同时走过一条河。”"),
        Tr::new("One rule runs on many threads at once. The river is the Warp River.", "一条规则同时在许多线程上跑。那条河就是 Warp River。"),
    ),
    n(
        Tr::new("\"I did not draw the city. I only wrote down which page it was on.\"", "“我没有画那座城，我只记下了它在哪一页。”"),
        Tr::new("Addressing: you store the tile's number, not the tile.", "寻址：存的是图块编号，不是图块本身。"),
    ),
    n(
        Tr::new("\"Thirty-two cats share one mouth. Whoever wants to speak passes a note first.\"", "“三十二只猫共用一张嘴，想说话的，得先递纸条。”"),
        Tr::new("Lockstep: lanes only talk to each other through SHFL and VOTE.", "锁步执行：车道之间只能用 SHFL / VOTE 传话。"),
    ),
    n(
        Tr::new("\"A file of cats marched in step. One turned aside, and the rest stood still to wait.\"", "“一队猫齐步走，有一只拐了弯，其余的站在原地等它。”"),
        Tr::new("Divergence and the mask: a branch runs both sides, one after the other.", "发散与掩码：一个分岔要把两边先后各走一遍。"),
    ),
    n(
        Tr::new("\"I reached for it and got yesterday's.\"", "“我伸手去拿，拿到的是昨天的东西。”"),
        Tr::new("A pipeline without interlocks reads the old value.", "没有互锁的流水线读到的是旧值。"),
    ),
    n(
        Tr::new("\"I pressed a tower flat onto paper. Every square asks: am I inside it?\"", "“我把一座塔压在一张纸上，每一格都在问：我在它里面吗？”"),
        Tr::new("Rasterising: each pixel tests the edge functions to see if it is in the triangle.", "光栅化：每个像素用边函数判断自己在不在三角形里。"),
    ),
    n(
        Tr::new("\"The stone runs fast, but it is always hungry.\"", "“石头跑得很快，可它总是饿着。”"),
        Tr::new("The memory wall: fast arithmetic is not enough if the data can't keep up.", "内存墙：算得快不够，数据要喂得上。"),
    ),
    n(
        Tr::new("\"Fetch side by side from one shelf and it is one trip. Each for itself, thirty-two.\"", "“同一排架子，并肩去取只要一趟；各拿各的，要走三十二趟。”"),
        Tr::new("Coalesced reads. Shared memory is a cache you manage yourself.", "合并访问。共享内存是自己管理的缓存。"),
    ),
    n(
        Tr::new("\"The colours the picture doesn't have are borrowed from the four squares nearby.\"", "“画上没有的颜色，是从旁边的四格里借来的。”"),
        Tr::new("Bilinear sampling: interpolate between neighbours.", "双线性采样：用邻居插值。"),
    ),
    n(
        Tr::new("\"A city split into squares, and each square only asks its neighbours.\"", "“一座城分成许多格，每格只问它的邻居。”"),
        Tr::new("Grids, thread blocks and stencil steps.", "网格、线程块与模板迭代。"),
    ),
    n(
        Tr::new("\"Every cat wanted to know how many stood ahead. A few questions, and they all knew.\"", "“每只猫都想知道前面站了几只，它们只问了几句就都知道了。”"),
        Tr::new("Scan: log steps give every prefix.", "扫描：log 步得到所有前缀。"),
    ),
    n(
        Tr::new("\"Some cats sleep and some run. As long as one is awake, the machine never stops.\"", "“有的猫在睡，有的猫在跑，只要总有一只醒着，机器就不会停。”"),
        Tr::new("Latency hiding: other warps fill the wait.", "延迟隐藏：靠别的 warp 填空。"),
    ),
    n(
        Tr::new("\"I no longer multiply one number at a time. I swallow the whole block.\"", "“我不再一个数一个数地乘，我一口吞下整块。”"),
        Tr::new("Tensor cores: one instruction, a whole tile of a matrix.", "张量核：一条指令一整块矩阵。"),
    ),
    n(
        Tr::new("\"To carry a little more, I gave the digits after the point to the wind.\"", "“为了多拿一点，我把小数点后面的几位送给了风。”"),
        Tr::new("Low precision buys bandwidth. Quantisation.", "低精度换带宽：量化。"),
    ),
    n(
        Tr::new("\"Many of me talk through a wall. One shout costs more than a day's work.\"", "“许多个我隔着墙说话，喊一句，比做一天的事还累。”"),
        Tr::new("Communication costs more than computation.", "通信比计算贵。"),
    ),
    n(
        Tr::new("\"Every cat in the city did only a little, and together they said one sentence.\"", "“一座城的猫，每一只只做了一点点，合起来说了一句话。”"),
        Tr::new("One model, cut across many machines, speaks.", "一个模型被切到许多台机器上说话。"),
    ),
];

/// Bai Yan's page for chapter `n` (1-based).
pub fn note(n: u8) -> &'static Note {
    &NOTES[(n.clamp(1, 16) - 1) as usize]
}
