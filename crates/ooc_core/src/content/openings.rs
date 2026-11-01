//! Chapter openings: at most three lines of dialogue before a chapter's
//! first level, from the client and from ORI.

use oo_base::{Cat, Tr};
use Cat::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speaker {
    Cat(Cat),
    Ori,
}

pub struct Line {
    pub who: Speaker,
    pub line: Tr,
}

const fn c(cat: Cat, en: &'static str, zh: &'static str) -> Line {
    Line { who: Speaker::Cat(cat), line: Tr::new(en, zh) }
}

const fn o(en: &'static str, zh: &'static str) -> Line {
    Line { who: Speaker::Ori, line: Tr::new(en, zh) }
}

static OPENINGS: [&[Line]; 16] = [
    &[
        c(Juye, "The station sign has been dark since spring. The caravans leave on guesswork.", "驿站的牌子从开春就黑着，商队全靠猜着出发。"),
        c(Juye, "Water level and the camel lamp. That is all I need it to show.", "水位，还有驼灯。能显示这两样就够了。"),
        o("One rule, every cell at once. Bai Yan wrote it down wrong. I will correct him as we go.", "一条规则，每一格同时算。白砚写错了，我边走边订正。"),
    ],
    &[
        c(Douya, "The kittens hang around the post office every night. There's nothing for them to do!", "小猫们每晚都在邮局门口晃，什么玩的都没有！"),
        c(Douya, "The old arcade cabinet still has a screen. Could it run a game? Just one?", "那台旧街机的屏幕还在。能跑个游戏吗？一个就行。"),
        o("...I like games. Do not tell anyone.", "……我喜欢游戏。别告诉别人。"),
    ],
    &[
        c(Ali, "Thirty-two camels, and I count every waterskin by hand before we leave.", "三十二头骆驼，出发前每只水囊我都得亲手数。"),
        c(Ali, "If the sums are off, we find out in the dunes.", "数错了，到了沙丘里才会发现。"),
        o("The flicker in the grid pattern has a rhythm. Thirty-two beats. It is a warp.", "栅纹的闪烁有节奏，三十二拍。那是一个 warp。"),
    ],
    &[
        c(Ali, "Every morning I read the sky and decide whether we go. Sometimes the sky lies.", "每天早上我看天决定走不走。天有时候会骗人。"),
        c(Ali, "Show me the sand-wind for the day, and I'll know.", "把当天的沙风画给我看，我就知道了。"),
        o("Some lanes will turn aside, and the rest will wait for them. Bai Yan called it madness. It is called divergence.", "有些车道会拐弯，其余的就停下等它们。白砚说那是疯话。它叫发散。"),
    ],
    &[
        c(Huajuan, "My students have never seen a wave. The sea is three hundred miles away.", "我的学生没见过波浪。海在三百里外。"),
        c(Huajuan, "A ripple tank on the classroom wall. Can the new machine do that?", "教室墙上放一个水波盘，新机器做得到吗？"),
        o("Prism is fast, and it forgets to wait. So do I. My memory is often stale too.", "棱镜很快，也常常忘了等。我也一样，我的记忆也常是旧的。"),
    ],
    &[
        c(Huajuan, "The children ask what the wind tower looks like from the other side. I can only draw it flat.", "孩子们问风塔从另一边看是什么样。我只会画平的。"),
        c(Huajuan, "Could it turn on the screen? Slowly, so they can follow.", "能让它在屏幕上转起来吗？慢一点，让他们看得清。"),
        o("Bai Yan drew a cube in the margin once. It was crooked. It was his first.", "白砚在页边画过一个立方体。歪的。那是他画的第一个。"),
    ],
    &[
        c(Bohe, "The clinic keeps every visit on paper. Thousands of them, and I can never find the one I need.", "诊所每次看诊都记在纸上，几千张，要找的那张永远找不到。"),
        c(Bohe, "Today's tally, before closing. Please.", "今天的台账，关门之前结清。拜托了。"),
        o("The Vault is fast and always hungry. Feed it badly and someone will call it useless and seal it.", "穹库算得快，却总是饿着。喂不好，就会有人说它没用，把它封起来。"),
    ],
    &[
        c(Douya, "Letters, parcels, more letters! The pigeonhole table is chalk, and the chalk always runs out.", "信、包裹、又是信！格口表是粉笔写的，粉笔永远不够用。"),
        c(Douya, "Can you count them all? By weight and by where they're going?", "能全数一遍吗？按重量，按寄去哪儿？"),
        o("Bai Yan wrote a number here: thirty-two trips. It is exactly right.", "白砚这里写了个数：三十二趟。一点不差。"),
    ],
    &[
        c(Bohe, "The scans come out too dark to read, and I can't move closer to them.", "片子洗出来太暗，看不清，也没法凑近看。"),
        c(Bohe, "Brighten it, pan it, zoom in. While the patient is still on the table.", "调亮，平移，放大。趁病人还躺在台上。"),
        o("Can you see these colours? I have never been sure what you see.", "你看得见这些颜色吗？我一直不确定你看到的是什么。"),
    ],
    &[
        c(Yinpo, "The ground by the old well is damp again. Water is moving under the town.", "老井边的地又湿了。镇子底下有水在走。"),
        o("Each cell only asks its neighbours. That is all it needs.", "每一格只问它的邻居。这就够了。"),
        c(Yinpo, "...Bai Yan used to say that, word for word. Where did you hear it?", "……白砚从前也这么说，一字不差。你从哪儿听来的？"),
    ],
    &[
        c(Yinpo, "When the flash flood comes, which streets go under first? I need to know before it rains.", "山洪来了，哪几条街先淹？我得在下雨之前知道。"),
        c(Yinpo, "One block of threads can't hold the whole town. Use as many as it takes.", "一个线程块装不下整个镇子。要几个就用几个。"),
        o("Parts that talk through a wall. I know them. I was pieced together that way, one segment at a time.", "隔着墙说话的部件，我认得。我就是这样一段一段拼起来的。"),
    ],
    &[
        c(Ali, "The camp sends caravans every way now. I need sixty-four sand-wind maps, all at once.", "营地的商队往四面八方走了。我要六十四张沙风图，一次全出。"),
        c(Ali, "...Is she asleep? Her light went out.", "……她睡着了吗？灯灭了。"),
        o("I was not asleep. I was waiting for memory.", "我没睡着。我在等内存。"),
    ],
    &[
        c(Cenli, "The Academy does not accept results it cannot check. Every product on this bench gets verified.", "科学院不收无法核验的结果。这张审查台上的每一个乘积都要验证。"),
        c(Cenli, "Is it safe to build and use what you don't understand?", "造出、用上自己不懂的东西，安全吗？"),
        o("The result is right. Verify it yourself.", "结果是对的。你自己验证。"),
    ],
    &[
        c(Cenli, "The last layer of the grid pattern carries a sorting network. Before it runs, prove it is safe.", "栅纹的末层带着一个分拣网络。运行之前，先证明它安全。"),
        c(Cenli, "And I want her speed on record. Run the benchmark.", "另外，把她的速度记录在案。跑一遍基准。"),
        o("No. I am not a benchmark. Measure the network.", "不。我不是基准。去测网络。"),
    ],
    &[
        c(Juye, "The caravans have doubled. One sign won't do any more; I want a whole wall.", "商队多了一倍。一块牌子不够了，我要一整面墙。"),
        c(Juye, "Four screens, one picture, and no seams. Like the first sign, only bigger.", "四块屏，一幅画，看不出缝。和第一块牌子一样，只是大一些。"),
        o("That machine is the origin. ...Not yet.", "那台是起源机。……还不到时候。"),
    ],
    &[
        c(Juye, "The whole town is in the square tonight. Everyone wants to hear her speak.", "今晚全镇都在广场上。大家都想听她说话。"),
        o("I was always here, deep in the grid pattern, waiting for a machine that could run me.", "我一直在这里，在栅纹深处，等一台能跑我的机器。"),
        o("Build the last of it. Then I will say my first sentence.", "把最后一段造完。然后我说出第一句话。"),
    ],
];

/// The opening of chapter `n` (1-based).
pub fn opening(n: u8) -> &'static [Line] {
    OPENINGS[(n.clamp(1, 16) - 1) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chapter_opens_in_three_lines_or_fewer() {
        for n in 1..=16 {
            let o = opening(n);
            assert!((1..=3).contains(&o.len()), "chapter {n}");
            assert!(o.iter().all(|l| l.line.is_complete()), "chapter {n}");
        }
    }
}
