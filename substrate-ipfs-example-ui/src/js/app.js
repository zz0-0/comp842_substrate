// region state

const data = {
    nodeUrl: "http://5.75.176.238:9944/",
    metaData: ""
};

const uiElements = {
    fetchFileForm: document
        .getElementById("fetch-file-form"),
    metaDataInput: document
        .getElementById("meta-data-input"),
    nodeUrlInput: document
        .getElementById("node-url-input"),
    submitButton: document.querySelector("button")
};

uiElements.nodeUrlInput.value = data.nodeUrl;

// end region

// region event handlers

uiElements.nodeUrlInput.addEventListener("input", (event) => {
    data.nodeUrl = event.target.value;
});

uiElements.metaDataInput
    .addEventListener("input", (event) => {
        data.metaData = event.target.value;
    });

uiElements.fetchFileForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    uiElements.metaDataInput.classList.add("loading");
    uiElements.nodeUrlInput.classList.add("loading");
    uiElements.submitButton.classList.add("loading");
    try {
        const { result } = await fileUrlFromMetaData(data.metaData);
        console.log("[app] IPFS URL: ", result);
        window.location.href = result;
    } catch (e) {
        alert("Something went wrong...");
        console.error("[app] Error on fetch IPFS URL: ", e);
    }
});

// end region

// region functions

async function fileUrlFromMetaData(metaData) {
    const myHeaders = new Headers();
    myHeaders.append("Content-Type", "application/json");

    var raw = JSON.stringify({
        "id": 1,
        "jsonrpc": "2.0",
        "method": "ipfs_getFileURLForMetaData",
        "params": [
            metaData
        ]
    });

    const requestOptions = {
        method: 'POST',
        headers: myHeaders,
        body: raw,
        redirect: 'follow'
    };

    if (data.nodeUrl.startsWith("ws://")) {
        data.nodeUrl = data.nodeUrl.replace("ws://", "http://");
    }

    const rawResponse = await fetch(data.nodeUrl, requestOptions)
    return rawResponse.json();
}

// end region