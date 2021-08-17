var eventSource = new EventSource('sse');

eventSource.onmessage = function(event) {
    console.log('Message from server ', event.data);
}
