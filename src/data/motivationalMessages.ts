/**
 * Motivational messages displayed during break screen
 * These messages encourage users to take proper breaks
 */

export const motivationalMessages = [
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

/**
 * Get a random motivational message
 * @returns A random message from the array
 */
export function getRandomMessage(): string {
  const index = Math.floor(Math.random() * motivationalMessages.length);
  const message = motivationalMessages[index];
  if (message === undefined) {
    throw new Error("motivationalMessages must contain at least one message.");
  }

  return message;
}
