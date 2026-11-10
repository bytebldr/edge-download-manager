// EDM YouTube Main World Helper
// Runs in the main page world context to access YouTube player API and window variables directly without CSP restrictions.
(function () {
  function getYouTubeStreamingData() {
    try {
      const player: any =
        document.getElementById("movie_player") ||
        document.querySelector(".html5-video-player");
      let streamData = null;
      let qualityLevels: string[] = [];

      if (player) {
        if (typeof player.getStreamingData === "function") {
          streamData = player.getStreamingData();
        } else if (typeof player.getPlayerResponse === "function") {
          streamData = player.getPlayerResponse()?.streamingData;
        }
        if (typeof player.getAvailableQualityLevels === "function") {
          qualityLevels = player.getAvailableQualityLevels() || [];
        }
      }

      if (!streamData && (window as any).ytInitialPlayerResponse?.streamingData) {
        streamData = (window as any).ytInitialPlayerResponse.streamingData;
      }

      return { streamData, qualityLevels };
    } catch {
      return { streamData: null, qualityLevels: [] };
    }
  }

  function syncToDOM(streamData: any, qualityLevels: string[]) {
    try {
      let el = document.getElementById("edm-yt-stream-cache");
      if (!el) {
        el = document.createElement("div");
        el.id = "edm-yt-stream-cache";
        el.style.display = "none";
        (document.body || document.documentElement).appendChild(el);
      }
      if (streamData) {
        el.setAttribute("data-stream", JSON.stringify(streamData));
      }
      if (qualityLevels && qualityLevels.length > 0) {
        el.setAttribute("data-qualities", JSON.stringify(qualityLevels));
      }
    } catch {}
  }

  function broadcastData() {
    const res = getYouTubeStreamingData();
    if (res.streamData || (res.qualityLevels && res.qualityLevels.length > 0)) {
      syncToDOM(res.streamData, res.qualityLevels);
      window.postMessage(
        {
          type: "EDM_STREAM_DATA_RESPONSE",
          payload: res.streamData,
          qualityLevels: res.qualityLevels,
        },
        "*"
      );
    }
    return res;
  }

  // Initial scan
  broadcastData();

  // Listen for request from EDM content script
  window.addEventListener("message", (event) => {
    if (event.source === window && event.data?.type === "EDM_REQUEST_STREAM_DATA") {
      const res = broadcastData();
      window.postMessage(
        {
          type: "EDM_STREAM_DATA_RESPONSE",
          payload: res.streamData,
          qualityLevels: res.qualityLevels,
        },
        "*"
      );
    }
  });

  // Post update whenever video changes (SPA navigation)
  window.addEventListener("yt-navigate-finish", () => {
    setTimeout(broadcastData, 200);
    setTimeout(broadcastData, 800);
  });
})();
