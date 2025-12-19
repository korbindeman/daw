<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let name = $state("");
  let greetMsg = $state("");

  async function greet(event: Event) {
    event.preventDefault();
    greetMsg = await invoke("greet", { name });
  }
</script>

<main class="min-h-screen bg-gray-50 dark:bg-gray-900 flex items-center justify-center p-4">
  <div class="max-w-md w-full space-y-8">
    <div class="text-center">
      <h1 class="text-4xl font-bold text-gray-900 dark:text-white mb-2">
        DAW Tauri
      </h1>
      <p class="text-gray-600 dark:text-gray-400">
        Digital Audio Workstation
      </p>
    </div>

    <form onsubmit={greet} class="space-y-4">
      <div>
        <input
          id="greet-input"
          type="text"
          placeholder="Enter a name..."
          bind:value={name}
          class="w-full px-4 py-2 border border-gray-300 dark:border-gray-700 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
        />
      </div>
      <button
        type="submit"
        class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded-lg transition-colors"
      >
        Greet
      </button>
    </form>

    {#if greetMsg}
      <p class="text-center text-gray-700 dark:text-gray-300 p-4 bg-white dark:bg-gray-800 rounded-lg">
        {greetMsg}
      </p>
    {/if}
  </div>
</main>
