/**
 * Motivational messages displayed during break screen
 * These messages encourage users to take proper breaks
 */

export const breakMotivationalMessages = [
  "目を休めましょう 👀",
  "少し立ち上がって歩きましょう 🚶",
  "深呼吸をしてリラックス 🌬️",
  "水分補給の時間です 💧",
  "肩を回してストレッチ 💪",
  "遠くを見て目を休めましょう 🌄",
  "軽く体を動かしましょう 🤸",
  "休憩は生産性の源です ⚡",
  "あなたの体は休息を必要としています 💚",
  "リフレッシュして戻ってきましょう ✨",
  "集中力を回復する時間です 🧠",
  "短い休憩が長期的なパフォーマンスを向上させます 📈",
];

function selectRandomBreakMotivationalMessage(
  messages: readonly string[],
): string {
  const index = Math.floor(Math.random() * messages.length);
  const message = messages[index];
  if (message === undefined) {
    throw new Error(
      "breakMotivationalMessages must contain at least one message.",
    );
  }

  return message;
}

export function getRandomBreakMotivationalMessage(): string {
  return selectRandomBreakMotivationalMessage(breakMotivationalMessages);
}
