const socket = new WebSocket("address-here");

socket.addEventListener("open", (event) => {
    socket.send(JSON.stringify({ SetName: "name" }));
});

socket.addEventListener("message", (event) => {
    const data = JSON.parse(event.data);
    socket.send(JSON.stringify({ Turn: "Clockwise" }));
});