// The demo video plays on its own, muted and in a loop. With reduced motion it stays on its
// poster and shows the player controls instead, so it only plays when asked.
(function () {
    var video = document.querySelector(".demo-video video");
    if (!video || !window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
        return;
    }
    video.removeAttribute("autoplay");
    video.pause();
    video.controls = true;
})();
