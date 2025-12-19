<script lang="ts">
    import Header from "$lib/components/Header.svelte";
    import { sessionStore } from "$lib/stores/session.svelte";
    import { transportStore } from "$lib/stores/transport.svelte";

    let loadError = $state<string | null>(null);

    const session = $derived(sessionStore.session);
    const loading = $derived(sessionStore.loading);
</script>

<div class="h-screen flex flex-col bg-gray-800">
    <Header />

    <main class="flex-1 overflow-hidden">
        {#if loading}
            <div class="h-full flex items-center justify-center text-white">
                <div class="text-center">
                    <div class="text-xl mb-2">Loading project...</div>
                    <div class="text-sm text-gray-400">Please wait</div>
                </div>
            </div>
        {:else if loadError}
            <div class="h-full flex items-center justify-center text-white">
                <div class="text-center max-w-md">
                    <div class="text-xl mb-2 text-red-400">
                        Failed to load project
                    </div>
                    <div class="text-sm text-gray-400 mb-4">{loadError}</div>
                    <button
                        onclick={() => window.location.reload()}
                        class="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded text-sm font-medium transition-colors"
                    >
                        Retry
                    </button>
                </div>
            </div>
        {:else if session}
            <div class="h-full flex items-center justify-center text-white">
                <div class="text-center">
                    <div class="text-2xl mb-4">
                        Project Loaded: {session.name}
                    </div>
                    <div class="text-sm text-gray-400 mb-6">
                        {session.tracks.length} tracks • {session.tempo} BPM • {session
                            .timeSignature.numerator}/{session.timeSignature
                            .denominator}
                    </div>

                    <!-- Transport controls placeholder -->
                    <div class="flex gap-2 justify-center">
                        <button
                            onclick={() => transportStore.play()}
                            class="px-4 py-2 bg-green-600 hover:bg-green-700 rounded text-sm font-medium transition-colors"
                        >
                            Play
                        </button>
                        <button
                            onclick={() => transportStore.pause()}
                            class="px-4 py-2 bg-yellow-600 hover:bg-yellow-700 rounded text-sm font-medium transition-colors"
                        >
                            Pause
                        </button>
                        <button
                            onclick={() => transportStore.stop()}
                            class="px-4 py-2 bg-red-600 hover:bg-red-700 rounded text-sm font-medium transition-colors"
                        >
                            Stop
                        </button>
                    </div>
                </div>
            </div>
        {:else}
            <div class="h-full flex items-center justify-center text-white">
                <div class="text-center">
                    <div class="text-xl mb-2">No project loaded</div>
                    <div class="text-sm text-gray-400">
                        Click "Open Project" to get started
                    </div>
                </div>
            </div>
        {/if}
    </main>
</div>
