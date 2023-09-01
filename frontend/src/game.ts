/**
 * Provides abstractions for reaching out to the game server, obtaining state, and
 * tracking state locally via a WASM module which tracks full game state.
 */
import init, { Client } from "generated/goat_wasm";

/** Initialize and create a game simulator. */
export async function simulator(): Promise<Client> {
    await init();
    return new Client();
}